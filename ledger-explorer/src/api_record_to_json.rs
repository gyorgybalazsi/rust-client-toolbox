use ledger_api::com::daml::ledger::api::v2::{Record, Value};
use serde_json::json;

pub fn api_record_to_json(record: &Record) -> serde_json::Value {
    let fields_json = record.fields.iter().map(|field| {
        let value_json = match &field.value {
            Some(val) => api_value_to_json(val),
            None => json!("Couldn't convert"),
        };
        (field.label.clone(), value_json)
    }).collect::<serde_json::Map<_, _>>();
    serde_json::Value::Object(fields_json)
}

fn api_value_to_json(value: &Value) -> serde_json::Value {
    match &value.sum {
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::Text(s)) => json!(s),
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::Int64(i)) => json!(i),
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::Bool(b)) => json!(b),
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::Numeric(n)) => json!(n),
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::Party(p)) => json!(p),
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::ContractId(cid)) => json!(cid),
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::Record(rec)) => api_record_to_json(rec),
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::Optional(opt)) => {
            match &opt.value {
                Some(inner) => api_value_to_json(inner),
                None => serde_json::Value::Null, // Use null for missing optionals
            }
        }
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::List(list)) => {
            let items: Vec<_> = list.elements.iter().map(api_value_to_json).collect();
            serde_json::Value::Array(items)
        }
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::TextMap(text_map)) => {
            let map: serde_json::Map<String, serde_json::Value> = text_map.entries.iter()
                .map(|entry| {
                    let value_json = match &entry.value {
                        Some(val) => api_value_to_json(val),
                        None => json!("Couldn't convert"),
                    };
                    (entry.key.clone(), value_json)
                })
                .collect();
            serde_json::Value::Object(map)
        }
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::GenMap(gen_map)) => {
            let arr: Vec<_> = gen_map.entries.iter()
                .map(|entry| {
                    json!({
                        "key": match &entry.key {
                            Some(key_val) => api_value_to_json(key_val),
                            None => json!("Couldn't convert"),
                        },
                        "value": match &entry.value {
                            Some(val) => api_value_to_json(val),
                            None => json!("Couldn't convert"),
                        }
                    })
                })
                .collect();
            serde_json::Value::Array(arr)
        }
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::Variant(variant)) => {
            json!({
                "constructor": &variant.constructor,
                "value": variant.value.as_ref().map(|v| api_value_to_json(&**v)).unwrap_or(json!("Couldn't convert"))
            })
        }
        Some(ledger_api::com::daml::ledger::api::v2::value::Sum::Enum(enum_val)) => {
            json!({
                "constructor": &enum_val.constructor
            })
        }
        _ => json!("Couldn't convert"),
    }
}

pub fn choice_argument_json(choice_argument: &Option<ledger_api::v2::Value>) -> serde_json::Value {
    match choice_argument {
        Some(value) => {
            if let Some(ledger_api::v2::value::Sum::Record(record)) = &value.sum {
                api_record_to_json(record)
            } else {
                json!("Couldn't convert")
            }
        }
        None => json!("Couldn't convert"),
    }
}


