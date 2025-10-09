use crate::lf_protobuf::com::daml::daml_lf_2::InternedDottedName;
use crate::lf_protobuf::com::daml::daml_lf_2::{BuiltinType, Type, r#type::Sum};

pub struct TypeRep {
    pub name: String,
    pub interned_type: Type,
}

pub fn resolve_type(
    typ: &Type,
    interned_types: &[Type],
    interned_strings: &[String],
    interned_dotted_names: &[InternedDottedName],
) -> String {
    match &typ.sum {
        Some(Sum::InternedType(idx)) => {
            if let Some(interned_type) = interned_types.get(*idx as usize) {
                resolve_type(
                    interned_type,
                    interned_types,
                    interned_strings,
                    interned_dotted_names,
                )
            } else {
                format!("<invalid interned type idx {}>", idx)
            }
        }
        Some(Sum::Con(con)) => {
            if let Some(tycon_id) = &con.tycon {
                let dotted_name = interned_dotted_names
                    .get(tycon_id.name_interned_dname as usize)
                    .unwrap();
                dbg!(&dotted_name);
                let name = interned_strings
                    .get(dotted_name.segments_interned_str[0] as usize)
                    .cloned()
                    .unwrap();
                name.to_string()
            } else {
                "<unknown Con>".to_string()
            }
        }
        Some(Sum::Builtin(builtin)) => std::convert::TryFrom::try_from(builtin.builtin)
            .map(|b: BuiltinType| format!("{:?}", b))
            .unwrap_or_else(|_| format!("<unknown builtin {}>", builtin.builtin)),
        Some(Sum::Var(var)) => {
            let name = interned_strings
                .get(var.var_interned_str as usize)
                .cloned()
                .unwrap_or_else(|| "<invalid>".to_string());
            name
        }
        Some(Sum::Struct(r#struct)) => {
            let fields: Vec<String> = r#struct
                .fields
                .iter()
                .map(|f| {
                    let fname = interned_strings
                        .get(f.field_interned_str as usize)
                        .cloned()
                        .unwrap_or_else(|| "<invalid>".to_string());
                    let ftype = f
                        .r#type
                        .as_ref()
                        .map(|t| {
                            resolve_type(t, interned_types, interned_strings, interned_dotted_names)
                        })
                        .unwrap_or_else(|| "<unknown>".to_string());
                    format!("{}: {}", fname, ftype)
                })
                .collect();
            format!("{{ {} }}", fields.join(", "))
        }
        _ => format!("{:?}", &typ.sum),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::package_from_dar;

    #[test]
    fn test_resolve_type() {
        let dar_path = "/Users/gyorgybalazsi/rust-client-toolbox/_daml/daml-ticketoffer/.daml/dist/daml-ticketoffer-0.0.1.dar";
        let package = package_from_dar(dar_path).expect("Failed to read package from DAR");

        let idx = 13; // Adjust this index based on your package interned types
        let interned_types = &package.interned_types;
        let interned_strings = &package.interned_strings;
        let interned_dotted_names = &package.interned_dotted_names;
        dbg!(&interned_types[idx]);
        // dbg!(&interned_dotted_names[313]);
        dbg!(&interned_strings[9]);
        let resolved = resolve_type(
            &interned_types[idx],
            interned_types,
            interned_strings,
            interned_dotted_names,
        );
        dbg!(resolved);
    }
}
