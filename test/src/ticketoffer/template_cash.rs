use ledger_api::v2::{
    Commands, Command, CreateCommand, Record,
    command_service_client::CommandServiceClient
};
use daml_type_rep::built_in_types::{DamlParty, DamlDecimal};
use daml_type_rep::template_id::TemplateId;
use client::submit_commands::submit_commands;
use anyhow::{anyhow, Result};
use derive_lapi_access::ToCreateArguments;
use client::submit_commands::CommandResult;
use derive_lapi_access::LapiAccess;
use daml_type_rep::lapi_access::LapiAccess;
use daml_type_rep::lapi_access::ToCreateArguments;

#[derive(Clone, Debug, serde::Serialize, ToCreateArguments)]
pub struct Cash {
    pub issuer: DamlParty,
    pub owner: DamlParty,
    pub amount: DamlDecimal,
}

impl Cash {
    pub fn new(issuer: DamlParty, owner: DamlParty, amount: DamlDecimal) -> Self {
        Cash { issuer, owner, amount }
    }
}

pub async fn create_cash(
    command_service_client: &mut CommandServiceClient<tonic::transport::Channel>,
    access_token: Option<&str>,
    user_id: Option<&str>,
    package_id: &str,
    issuer: String,
    owner: String,
    amount: f64,
) -> Result<String> {
    let create_cash_command = CreateCommand {
        template_id: Some(TemplateId::new(
            package_id,
            "Main",
            "Cash",
        ).to_template_id()),
        create_arguments: Some(Cash::new(DamlParty::new(&issuer), DamlParty::new(&owner), DamlDecimal::new(amount)).to_create_arguments()),
    };

    let commands = Commands {
        act_as: vec![issuer.clone()],
        commands: vec![Command {
            command: Some(ledger_api::v2::command::Command::Create(create_cash_command)),
        }],
        user_id: user_id.unwrap_or("").to_string(),
        command_id: format!("command-{}", uuid::Uuid::new_v4()),
        ..Default::default()
    };

    let result = submit_commands(command_service_client, access_token, commands).await?;
    let contract_id = if let Some(CommandResult::ContractId(id)) = result.get(0) {
        id.clone()
    } else {
        return Err(anyhow!("No contract id found in create_cash result"));
    };

    Ok(contract_id)
}

#[derive(serde::Serialize, LapiAccess)]
/// Represents the Transfer choice on the Cash template.
pub struct Transfer {
    pub new_owner: DamlParty,
}

impl Transfer {
    pub fn new(new_owner: DamlParty) -> Self {
        Transfer { new_owner }
    }
}

pub async fn exercise_transfer(
    command_service_client: &mut CommandServiceClient<tonic::transport::Channel>,
    access_token: Option<&str>,
    user_id: Option<&str>,
    package_id: &str,
    contract_id: String,
    new_owner: String,
    current_owner: String,
) -> Result<()> {
    let exercise_command = ledger_api::v2::ExerciseCommand {
        template_id: Some(TemplateId::new(
            package_id,
            "Main",
            "Cash",
        ).to_template_id()),
        contract_id,
        choice: "Transfer".to_string(),
        choice_argument: Some(Transfer::new(DamlParty::new(&new_owner)).to_lapi_value()),
        ..Default::default()
    };

    let commands = Commands {
        act_as: vec![current_owner],
        user_id: user_id.unwrap_or("").to_string(),
        commands: vec![Command {
            command: Some(ledger_api::v2::command::Command::Exercise(exercise_command)),
        }],
        command_id: format!("command-{}", uuid::Uuid::new_v4()),
        ..Default::default()
    };

    submit_commands(
        command_service_client,
        access_token,
        commands,
    ).await?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use client::jwt::fake_jwt_for_user;
    use client::parties::get_parties;
    use client::testutils::daml_start;
    use tokio;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_create_and_transfer_cash() -> Result<()> {
        tracing_subscriber::fmt::init();
        let sandbox_port = 6865;
        let url = format!("http://localhost:{}", sandbox_port);
        let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        // Use PathBuf for package_root
        let package_root = PathBuf::from(&crate_root)
            .join("..")
            .join("_daml")
            .join("daml-ticketoffer")
            .canonicalize()
            .unwrap();

        let _guard = daml_start(package_root.clone(), sandbox_port).await?;

        // Setup test values
        let package_id = "#daml-ticketoffer".to_string();

        let alice_user = "aliceuser";
        let alice_token = fake_jwt_for_user(alice_user);
        let alice_parties =
            get_parties(url.clone(), Some(&alice_token), Some("Alice".to_string())).await?;

        let scrooge_bank_user = "scroogebankuser";
        let scrooge_bank_token = fake_jwt_for_user(scrooge_bank_user);
        let scrooge_bank_parties =
            get_parties(url.clone(), Some(&scrooge_bank_token), Some("ScroogeBank".to_string())).await?;

        let ticketwizard_user = "ticketwizarduser";
        let ticketwizard_token = fake_jwt_for_user(ticketwizard_user);
        let ticketwizard_parties =
            get_parties(url.clone(), Some(&ticketwizard_token), Some("TicketWizard".to_string())).await?;

        let issuer = scrooge_bank_parties
            .get(0)
            .cloned()
            .unwrap();
        let owner = alice_parties
            .get(0)
            .cloned()
            .unwrap();
        let amount = 10.5_f64;
        let new_owner = ticketwizard_parties
            .get(0)
            .cloned()
            .unwrap();

        // Connect to ledger
        let channel = tonic::transport::Channel::from_shared(url)
            .unwrap()
            .connect()
            .await
            .unwrap();
        let mut command_service_client = CommandServiceClient::new(channel);

        // Create cash
        let create_result = create_cash(
            &mut command_service_client,
            Some(alice_token.as_str()),
            Some(alice_user),
            &package_id,
            issuer.clone(),
            owner.clone(),
            amount,
        )
        .await;

        assert!(
            create_result.is_ok(),
            "Cash creation failed: {:?}",
            create_result
        );

        let contract_id = create_result.unwrap();

        // Transfer cash
        let transfer_result = exercise_transfer(
            &mut command_service_client,
            Some(alice_token.as_str()),
            Some(alice_user),
            &package_id,
            contract_id,
            new_owner.clone(),
            owner.clone(),
        )
        .await;

        assert!(transfer_result.is_ok(), "Transfer cash failed: {:?}", transfer_result);

        Ok(())
    }
}