use anyhow::{Result, anyhow};
use client::submit_commands::CommandResult;
use client::submit_commands::submit_commands;
use daml_type_rep::built_in_types::{DamlInt, DamlMap, DamlOptional, DamlParty, DamlText};
use daml_type_rep::lapi_access::LapiAccess;
use daml_type_rep::lapi_access::ToCreateArguments;
use daml_type_rep::template_id::TemplateId;
use derive_lapi_access::LapiAccess;
use derive_lapi_access::ToCreateArguments;
use ledger_api::v2::Record;
use ledger_api::v2::{
    Command, Commands, CreateCommand, ExerciseCommand,
    command_service_client::CommandServiceClient, value::Sum,
};
use std::collections::BTreeMap;
use tracing::info;

#[derive(Debug, serde::Serialize, Clone, LapiAccess)]
pub enum Color {
    Red,
    Green,
    Blue,
}

#[derive(Debug, serde::Serialize, Clone, LapiAccess)]
pub enum Price {
    USD { amount: DamlInt, color: Color },
    EUR { amount: DamlInt, color: Color },
    GBP,
}

#[derive(Debug, serde::Serialize, Clone, LapiAccess)]
pub struct Coordinates {
    x: DamlInt,
    y: DamlInt,
    rgb: Rgb,
}

#[derive(Debug, serde::Serialize, Clone, LapiAccess)]
pub struct Rgb {
    red: DamlInt,
    green: DamlInt,
    blue: DamlInt,
}

#[derive(Debug, serde::Serialize, ToCreateArguments, LapiAccess)]
pub struct Asset {
    issuer: DamlParty,
    owner: DamlParty,
    name: DamlText,
    price: Price,
    color: Color,
    coordinates: Coordinates,
    mapping: DamlMap<DamlText, DamlInt>,
    maybeDescription: DamlOptional<DamlText>,
}

impl Asset {
    pub fn new(
        issuer: String,
        owner: String,
        name: String,
        price: Price,
        color: Color,
        coordinates: Coordinates,
        mapping: BTreeMap<String, i64>,
        maybe_description: Option<String>,
    ) -> Self {
        Asset {
            issuer: DamlParty::new(&issuer),
            owner: DamlParty::new(&owner),
            name: DamlText::new(name),
            price,
            color,
            coordinates,
            mapping: DamlMap::new(
                mapping
                    .into_iter()
                    .map(|(k, v)| (DamlText::new(k), DamlInt::new(v)))
                    .collect(),
            ),
            maybeDescription: DamlOptional::new(maybe_description.map(|desc| DamlText::new(desc))),
        }
    }
}

pub async fn create_asset(
    command_service_client: &mut CommandServiceClient<tonic::transport::Channel>,
    access_token: Option<&str>,
    user_id: Option<&str>,
    package_id: String,
    issuer: String,
    owner: String,
    name: String,
    price: Price,
    color: Color,
    coordinates: Coordinates,
    mapping: BTreeMap<String, i64>,
    maybe_description: Option<String>,
) -> Result<String> {
    let create_asset_command = CreateCommand {
        template_id: Some(TemplateId::new(&package_id, "Main", "Asset").to_template_id()),
        create_arguments: Some(
            Asset::new(
                issuer.clone(),
                owner.clone(),
                name.clone(),
                price.clone(),
                color.clone(),
                coordinates.clone(),
                mapping.clone(),
                maybe_description.clone(),
            )
            .to_create_arguments(),
        ),
    };

    let commands = Commands {
        act_as: vec![issuer.clone()],
        commands: vec![Command {
            command: Some(ledger_api::v2::command::Command::Create(
                create_asset_command,
            )),
        }],
        user_id: user_id.unwrap_or("").to_string(),
        command_id: format!("command-{}", uuid::Uuid::new_v4()),
        ..Default::default()
    };

    let result = submit_commands(command_service_client, access_token, commands).await?;
    let contract_ids = result
        .iter()
        .filter_map(|r| {
            if let CommandResult::ContractId(id) = r {
                Some(id.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if contract_ids.len() == 1 {
        Ok(contract_ids[0].clone())
    } else {
        Err(anyhow!(
            "Expected exactly one contract id, found {}",
            contract_ids.len()
        ))
    }
}

pub async fn exercise_give(
    command_service_client: &mut CommandServiceClient<tonic::transport::Channel>,
    access_token: Option<&str>,
    user_id: Option<&str>,
    package_id: String,
    contract_id: String,
    current_owner: String,
    new_owner: String,
) -> Result<String> {
    let exercise_command = ExerciseCommand {
        template_id: Some(TemplateId::new(&package_id, "Main", "Asset").to_template_id()),
        contract_id: contract_id.clone(),
        choice: "Give".to_string(),
        choice_argument: Some(LapiAccess::to_lapi_value(&Give::new(new_owner.clone()))),
        ..Default::default()
    };

    let commands = Commands {
        act_as: vec![current_owner.clone()],
        user_id: user_id.unwrap_or("").to_string(),
        commands: vec![Command {
            command: Some(ledger_api::v2::command::Command::Exercise(exercise_command)),
        }],
        command_id: format!("command-{}", uuid::Uuid::new_v4()),
        ..Default::default()
    };

    let result = submit_commands(command_service_client, access_token, commands).await?;
    let contract_ids = result
        .iter()
        .filter_map(|r| {
            if let CommandResult::ContractId(id) = r {
                Some(id.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if contract_ids.len() == 1 {
        Ok(contract_ids[0].clone())
    } else {
        Err(anyhow!(
            "Expected exactly one contract id, found {}",
            contract_ids.len()
        ))
    }
}

pub async fn exercise_get_view(
    command_service_client: &mut CommandServiceClient<tonic::transport::Channel>,
    access_token: Option<&str>,
    user_id: Option<&str>,
    package_id: String,
    contract_id: String,
    owner: String,
) -> Result<()> {
    let exercise_command = ExerciseCommand {
        template_id: Some(TemplateId::new(&package_id, "Main", "Asset").to_template_id()),
        contract_id: contract_id.clone(),
        choice: "GetView".to_string(),
        choice_argument: Some(ledger_api::v2::Value {
            sum: Some(Sum::Record(Record {
                record_id: None,
                fields: vec![],
            })),
        }),
        ..Default::default()
    };

    let commands = Commands {
        act_as: vec![owner.clone()],
        user_id: user_id.unwrap_or("").to_string(),
        commands: vec![Command {
            command: Some(ledger_api::v2::command::Command::Exercise(exercise_command)),
        }],
        command_id: format!("command-{}", uuid::Uuid::new_v4()),
        ..Default::default()
    };

    let result = submit_commands(command_service_client, access_token, commands).await?;
    info!("Length of result: {}", result.len());
    if let Some(CommandResult::ExerciseResult(value)) = result.get(0) {
        info!("Exercise GetView result: {:#?}", value);
        info!(
            "Exercise GetView result extracted from LAPI value: {:#?}",
            Asset::from_lapi_value(value)
        );
    }
    Ok(())
}

#[derive(serde::Serialize, LapiAccess)]
pub struct Give {
    new_owner: DamlParty,
}

impl Give {
    pub fn new(new_owner: String) -> Self {
        Give {
            new_owner: DamlParty::new(&new_owner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use client::jwt::fake_jwt_for_user;
    use client::parties::get_parties;
    use client::testutils::daml_start;
    use tokio;
    use tracing::info;
    use tracing_subscriber::EnvFilter;

    #[tokio::test]
    async fn test_create_and_give_asset_full() -> Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new("debug")) // or "debug", "trace", etc.
            .init();
        let sandbox_port = 6865;
        let url = format!("http://localhost:{}", sandbox_port);
        let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let package_root = std::path::PathBuf::from(&crate_root)
            .join("..")
            .join("_daml")
            .join("full")
            .canonicalize()
            .expect("Failed to canonicalize package_root");

        info!("Starting DAML sandbox at {}", package_root.display());

        let _guard = daml_start(package_root, sandbox_port).await?;

        // Setup test values
        let package_id = "#full".to_string();
        // let package_id = "86af146d525686523ff61402624ebb46bdbd8274f6c35391ebfd573667cf4f6c".to_string();

        let alice_user = "alice_user";
        let alice_token = fake_jwt_for_user(alice_user);
        let alice_parties =
            get_parties(url.clone(), Some(&alice_token), Some("Alice".to_string())).await?;
        let alice_party = alice_parties.get(0).cloned().unwrap();

        let bob_parties =
            get_parties(url.clone(), Some(&alice_token), Some("Bob".to_string())).await?;
        let bob_party = bob_parties.get(0).cloned().unwrap();
        let bob_user = "bob_user";
        let bob_token = fake_jwt_for_user(bob_user);

        let price = Price::USD {
            amount: DamlInt::new(100),
            color: Color::Red,
        };
        let color = Color::Red;

        // Connect to ledger
        let channel = tonic::transport::Channel::from_shared(url)
            .unwrap()
            .connect()
            .await
            .unwrap();
        let mut command_service_client = CommandServiceClient::new(channel);

        // Create asset
        let create_result = create_asset(
            &mut command_service_client,
            Some(alice_token.as_str()),
            Some(alice_user),
            package_id.clone(),
            alice_party.clone(), // issuer
            alice_party.clone(), // owner
            "Test asset".to_string(),
            price.clone(),
            color.clone(),
            Coordinates {
                x: DamlInt::new(10),
                y: DamlInt::new(20),
                rgb: Rgb {
                    red: DamlInt::new(255),
                    green: DamlInt::new(0),
                    blue: DamlInt::new(0),
                },
            },
            BTreeMap::from([
                ("a".to_string(), 1),
                ("b".to_string(), 2),
                ("c".to_string(), 3),
            ]),
            Some("A nice TV".to_string()),
        )
        .await;

        assert!(
            create_result.is_ok(),
            "Asset creation failed: {:?}",
            create_result
        );
        let created_contract_id = create_result.unwrap();

        // Give asset
        let give_result = exercise_give(
            &mut command_service_client,
            Some(alice_token.as_str()),
            Some(alice_user),
            package_id.clone(),
            created_contract_id.clone(),
            alice_party.clone(), // current_owner
            bob_party.clone(),   // new_owner
        )
        .await;

        assert!(give_result.is_ok(), "Give asset failed: {:?}", give_result);
        let given_contract_id = give_result.unwrap();

        // Exercise GetView
        let get_view_result = exercise_get_view(
            &mut command_service_client,
            Some(bob_token.as_str()),
            Some(bob_user),
            package_id,
            given_contract_id,
            bob_party.clone(), // owner
        )
        .await;

        assert!(
            get_view_result.is_ok(),
            "GetView exercise failed: {:?}",
            get_view_result
        );

        Ok(())
    }
}
