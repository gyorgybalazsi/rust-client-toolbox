use crate::daml_custom_data_type_reps::record::DamlRecordRep;
use anyhow::{Context, Result};
use proc_macro2::Ident;
use quote::quote;
use std::convert::TryFrom;
use std::fs::File;
use std::io::Write;

/// Given a DAR file path, extracts DALF, converts DefDataType items to DamlRecordRep,
/// generates Rust struct definitions, and writes them to a file.
pub fn generate_rust_structs_from_dar(dar_path: &str, output_path: &str) -> Result<()> {
    // Extract the package from the DAR file
    let package = crate::package::package_from_dar(dar_path)
        .with_context(|| format!("Failed to read package from '{}'", dar_path))?;

    let mut output = File::create(output_path)
        .with_context(|| format!("Failed to create output file '{}'", output_path))?;

    for module in &package.modules {
        for def_data_type in &module.data_types {
            // Try to convert DefDataType to DamlRecordRep
            if let Ok(record_rep) = DamlRecordRep::try_from((def_data_type, module, &package)) {
                // Generate Rust struct code
                let struct_code = rust_struct_from_daml_record_rep(&record_rep);
                writeln!(output, "{}", struct_code)
                    .with_context(|| "Failed to write struct to output file")?;
            }
        }
    }
    Ok(())
}

/// Sanitizes a string to a valid Rust identifier
fn sanitize_ident(name: &str) -> Ident {
    let mut s = name.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    if !s
        .chars()
        .next()
        .map(|c| c.is_ascii_alphabetic() || c == '_')
        .unwrap_or(false)
    {
        s = format!("_{}", s);
    }
    Ident::new(&s, proc_macro2::Span::call_site())
}

/// Generates Rust struct code from a DamlRecordRep using the quote! macro and prettyplease for formatting
fn rust_struct_from_daml_record_rep(record: &DamlRecordRep) -> String {
    let struct_name = sanitize_ident(&record.record_name);
    let field_names: Vec<Ident> = record
        .fields
        .iter()
        .map(|f| sanitize_ident(&f.field_name))
        .collect();
    let field_types: Vec<Ident> = record
        .fields
        .iter()
        .map(|f| sanitize_ident(&f.type_name))
        .collect();

    let struct_tokens = quote!(
        #[derive(Debug, Clone)]
        pub struct #struct_name {
            #( pub #field_names: #field_types, )*
        }
    );

    let syntax_tree = syn::parse2(struct_tokens).expect("Failed to parse tokens to syntax tree");
    prettyplease::unparse(&syntax_tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::info;

    #[test]
    fn test_generate_rust_structs_from_dar() {
        tracing_subscriber::fmt()
            .init();
        let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        info!("Crate root: {}", crate_root);
        let dar_path = std::path::PathBuf::from(&crate_root)
            .join("..")
            .join("_daml")
            .join("daml-ticketoffer")
            .join(".daml")
            .join("dist")
            .join("daml-ticketoffer-0.0.1.dar")
            .canonicalize()
            .expect("Failed to canonicalize package_root");
        let output_path = std::path::PathBuf::from(&crate_root)
            .join("generated")
            .join("ticketoffer_structs.rs");
        let result = generate_rust_structs_from_dar(
            dar_path.to_str().expect("DAR path is not valid UTF-8"),
            output_path.to_str().expect("Output path is not valid UTF-8"),
        );
        assert!(
            result.is_ok(),
            "Failed to generate Rust structs from DAR: {:?}",
            result.err()
        );
        // Optionally, check that the output file exists and is not empty
        let contents = std::fs::read_to_string(output_path).expect("Output file not found");
        assert!(!contents.is_empty(), "Output file is empty");
    }
}
