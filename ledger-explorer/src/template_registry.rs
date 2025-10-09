// use std::collections::HashMap;
// use ledger_api::v2::Record;

// use client::daml_model_rep::asset::template_asset::{Asset, Give};
// use client::daml_model_rep::ticketoffer::template_cash::{Cash, Transfer};
// use client::daml_model_rep::ticketoffer::template_ticketoffer::{TicketOffer, Accept};
// use client::daml_model_rep::ticketoffer::template_ticketagreement::TicketAgreement;
// use client::daml_model_rep::splice::amulet::amulet::Amulet;
// use client::daml_model_rep::splice::validator_license::validator_liveness_activity_record::ValidatorLivenessActivityRecord;

// // Type alias for the conversion function
// type FromApiRecordFn = fn(&Record) -> anyhow::Result<serde_json::Value>;


// // Helper wrappers for JSON conversion
// fn asset_from_api_record(record: &Record) -> anyhow::Result<serde_json::Value> {
//     Asset::from_api_record(record).and_then(|a| serde_json::to_value(a).map_err(Into::into))
// }
// fn cash_from_api_record(record: &Record) -> anyhow::Result<serde_json::Value> {
//     Cash::from_api_record(record).and_then(|c| serde_json::to_value(c).map_err(Into::into))
// }
// fn ticketoffer_from_api_record(record: &Record) -> anyhow::Result<serde_json::Value> {
//     TicketOffer::from_api_record(record).and_then(|t| serde_json::to_value(t).map_err(Into::into))
// }
// fn ticket_agreement_from_api_record(record: &Record) -> anyhow::Result<serde_json::Value> {
//     TicketAgreement::from_api_record(record).and_then(|t| serde_json::to_value(t).map_err(Into::into))
// }
// fn amulet_from_api_record(record: &Record) -> anyhow::Result<serde_json::Value> {
//     Amulet::from_api_record(record).and_then(|a| serde_json::to_value(a).map_err(Into::into))
// }
// fn validator_liveness_activity_record_from_api_record(record: &Record) -> anyhow::Result<serde_json::Value> {
//     ValidatorLivenessActivityRecord::from_api_record(record).and_then(|v| serde_json::to_value(v).map_err(Into::into))
// }

// /// Returns a mapping from "module.template" to the corresponding from_api_record function
// pub fn get_template_registry() -> HashMap<&'static str, FromApiRecordFn> {
//     let mut map: HashMap<&'static str, FromApiRecordFn> = HashMap::new();
//     map.insert("Main.Asset", asset_from_api_record);
//     map.insert("Main.Cash", cash_from_api_record);
//     map.insert("Main.TicketOffer", ticketoffer_from_api_record);
//     map.insert("Main.TicketAgreement", ticket_agreement_from_api_record);
//     map.insert("Splice.Amulet.Amulet", amulet_from_api_record);
//     map.insert("Splice.ValidatorLicense.ValidatorLivenessActivityRecord", validator_liveness_activity_record_from_api_record);
//     map
// }

// type FromApiValueFn = fn(&ledger_api::v2::Value) -> anyhow::Result<serde_json::Value>;

// fn give_from_api_value(value: &ledger_api::v2::Value) -> anyhow::Result<serde_json::Value> {
//     Give::from_api_value(value).and_then(|g| serde_json::to_value(g).map_err(Into::into))
// }

// fn transfer_from_api_value(value: &ledger_api::v2::Value) -> anyhow::Result<serde_json::Value>{
//     Transfer::from_api_value(value).and_then(|t| serde_json::to_value(t).map_err(Into::into))
// }

// fn accept_from_api_value(value: &ledger_api::v2::Value) -> anyhow::Result<serde_json::Value> {
//     Accept::from_api_value(value).and_then(|a| serde_json::to_value(a).map_err(Into::into))
// }

// fn archive_from_api_value(_value: &ledger_api::v2::Value) -> anyhow::Result<serde_json::Value> {
//     Ok(serde_json::json!({}))
// }

// // TODO prefix choice names wit module for uniqueness
// pub fn get_choice_registry() -> HashMap<&'static str, FromApiValueFn> {
//     let mut map: HashMap<&'static str, FromApiValueFn> = HashMap::new();
//     map.insert("Give", give_from_api_value);
//     map.insert("Transfer", transfer_from_api_value);
//     map.insert("Accept", accept_from_api_value);
//     map.insert("Archive", archive_from_api_value);
//     map
// }