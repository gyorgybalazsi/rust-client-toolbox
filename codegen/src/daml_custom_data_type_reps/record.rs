use crate::lf_protobuf::com::daml::daml_lf_2::DefDataType;
use crate::lf_protobuf::com::daml::daml_lf_2::Module;
use crate::lf_protobuf::com::daml::daml_lf_2::Package; // <-- Add this import
use crate::lf_protobuf::com::daml::daml_lf_2::def_data_type::DataCons::Record;
use crate::resolve_type::resolve_type;
use anyhow::{Context, Ok, Result, bail};
use std::convert::TryFrom;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DamlRecordRep {
    pub module_name: String,
    pub record_name: String,
    pub fields: Vec<DamlRecordFieldRep>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TemplateRep {
    pub record: DamlRecordRep,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChoiceRep {
    pub record: DamlRecordRep,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DamlRecordFieldRep {
    pub field_name: String,
    pub type_name: String,
}

impl<'a> TryFrom<(&'a DefDataType, &'a Module, &'a Package)> for DamlRecordRep {
    type Error = anyhow::Error;

    fn try_from(
        (def_data_type, module, package): (&'a DefDataType, &'a Module, &'a Package),
    ) -> Result<Self> {
        let module_name = module_name(module, package)?;
        let name = def_data_type_name(def_data_type, package)?;
        let fields = def_data_type_record_fields(def_data_type, package)?;
        Ok(DamlRecordRep {
            module_name,
            record_name: name,
            fields,
        })
    }
}

fn module_name(module: &Module, package: &Package) -> Result<String> {
    let interned_strings = &package.interned_strings;
    let interned_dotted_names = &package.interned_dotted_names;

    let dotted_name = interned_dotted_names
        .get(module.name_interned_dname as usize)
        .context("module.name_interned_dname not found in interned_dotted_names")?;

    let name = interned_strings
        .get(dotted_name.segments_interned_str[0] as usize)
        .cloned()
        .context("module_interned_dotted_name not found in interned_strings")?;

    Ok(name)
}
fn def_data_type_name(def_data_type: &DefDataType, package: &Package) -> Result<String> {
    let interned_strings = &package.interned_strings;
    let interned_dotted_names = &package.interned_dotted_names;

    let dotted_name = interned_dotted_names
        .get(def_data_type.name_interned_dname as usize)
        .context("def_data_type.name_interned_dname not found in interned_dotted_names")?;

    let name = interned_strings
        .get(dotted_name.segments_interned_str[0] as usize)
        .cloned()
        .context("def_data_type_interned_dotted_name not found in interned_strings")?;

    Ok(name)
}

#[allow(unused)]
fn def_data_type_is_record(def_data_type: &DefDataType, package: &Package) -> Result<bool> {
    if let Some(Record(record)) = &def_data_type.data_cons {
        Ok(true)
    } else {
        Ok(false)
    }
}

fn def_data_type_record_fields(
    def_data_type: &DefDataType,
    package: &Package,
) -> Result<Vec<DamlRecordFieldRep>> {
    let interned_strings = &package.interned_strings;
    let interned_types = &package.interned_types;

    if let Some(Record(record)) = &def_data_type.data_cons {
        let fields = record.fields.iter().map(|field| {
            let field_name = interned_strings
                .get(field.field_interned_str as usize)
                .cloned()
                .unwrap_or_else(|| "<invalid>".to_string());
            let field_type = field.r#type.as_ref().map_or_else(
                || "<unknown type>".to_string(),
                |typ| resolve_type(typ, interned_types, interned_strings, &[]),
            );
            (field_name, field_type)
        });
        Ok(fields
            .map(|(field_name, field_type)| DamlRecordFieldRep {
                field_name,
                type_name: field_type,
            })
            .collect())
    } else {
        bail!("Data type is not a record");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result};
    #[test]
    fn test_try_convert_data_types_to_daml_record_rep() -> Result<()> {
        let dar_path = "/Users/gyorgybalazsi/rust-client-toolbox/_daml/daml-ticketoffer/.daml/dist/daml-ticketoffer-0.0.1.dar";
        let package = crate::package::package_from_dar(dar_path)
            .with_context(|| format!("Failed to read package from '{}'", dar_path))?;

        let module = package.modules.get(0).context("No modules in package")?;
        for def_data_type in &module.data_types {
            match DamlRecordRep::try_from((def_data_type, module, &package)) {
                Result::Ok(record_rep) => {
                    println!("Successfully converted: {:#?}", record_rep);
                }
                Result::Err(e) => {
                    println!("Failed to convert: {}", e);
                }
            }
        }
        Ok(())
    }
}
