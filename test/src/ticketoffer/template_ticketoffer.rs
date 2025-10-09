use daml_type_rep::template_id::TemplateId;
use client::submit_commands::submit_commands;
use ledger_api::v2::{
    Command, Commands, CreateCommand, command_service_client::CommandServiceClient,
};
use ledger_api::v2::Record;
use anyhow::{anyhow, Result};
use derive_lapi_access::{ToCreateArguments, LapiAccess};
use daml_type_rep::built_in_types::{DamlContractId, DamlDecimal, DamlParty};
use client::submit_commands::CommandResult;
use daml_type_rep::lapi_access::LapiAccess;
use daml_type_rep::lapi_access::ToCreateArguments;

#[derive(Clone, Debug, serde::Serialize, ToCreateArguments)]
pub struct TicketOffer {
    pub organizer: DamlParty,
    pub buyer: DamlParty,
    pub price: DamlDecimal,
}

impl TicketOffer {
    pub fn new(organizer: DamlParty, buyer: DamlParty, price: DamlDecimal) -> Self {
        TicketOffer {
            organizer,
            buyer,
            price,
        }
    }
}

#[derive(serde::Serialize,LapiAccess)]
/// Represents the Accept choice on the TicketOffer template.
pub struct Accept {
    pub cash_id: DamlContractId,
}

impl Accept {
    pub fn new(cash_id: String) -> Self {
        Accept { cash_id: DamlContractId::new(&cash_id) }
    }
}

pub async fn create_ticketoffer(
    command_service_client: &mut CommandServiceClient<tonic::transport::Channel>,
    access_token: Option<&str>,
    user_id: Option<&str>,
    package_id: &str,
    organizer: String,
    buyer: String,
    price: f64,
) -> Result<String> {
    let create_ticketoffer_command = CreateCommand {
        template_id: Some(TemplateId::new(package_id, "Main", "TicketOffer").to_template_id()),
        create_arguments: Some(
            TicketOffer::new(DamlParty::new(&organizer), DamlParty::new(&buyer), DamlDecimal::new(price)).to_create_arguments(),
        ),
    };

    let commands = Commands {
        act_as: vec![organizer.clone()],
        user_id: user_id.unwrap_or("").to_string(),
        commands: vec![Command {
            command: Some(ledger_api::v2::command::Command::Create(
                create_ticketoffer_command,
            )),
        }],
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

pub async fn exercise_accept(
    command_service_client: &mut CommandServiceClient<tonic::transport::Channel>,
    access_token: Option<&str>,
    user_id: Option<&str>,
    package_id: &str,
    contract_id: String,
    cash_id: String,
    buyer: String,
) -> Result<()> {
    let exercise_command = ledger_api::v2::ExerciseCommand {
        template_id: Some(TemplateId::new(package_id, "Main", "TicketOffer").to_template_id()),
        contract_id,
        choice: "Accept".to_string(),
        choice_argument: Some(Accept::new(cash_id).to_lapi_value()),
        ..Default::default()
    };

    let commands = Commands {
        act_as: vec![buyer],
        user_id: user_id.unwrap_or("").to_string(),
        commands: vec![Command {
            command: Some(ledger_api::v2::command::Command::Exercise(exercise_command)),
        }],
        command_id: format!("command-{}", uuid::Uuid::new_v4()),
        ..Default::default()
    };

    submit_commands(command_service_client, access_token, commands).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use client::jwt::fake_jwt_for_user;
    use client::parties::get_parties;
    use client::testutils::daml_start;
    use tokio;
    use crate::ticketoffer::template_cash::create_cash;

    #[tokio::test]
    async fn test_create_and_accept_ticketoffer() -> Result<()> {
        tracing_subscriber::fmt::init();
        let sandbox_port = 7000;
        let url = format!("http://localhost:{}", sandbox_port);
        let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        // Use PathBuf for package_root
        let package_root = std::path::PathBuf::from(&crate_root)
            .join("..")
            .join("_daml")
            .join("daml-ticketoffer");

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
        let organizer = ticketwizard_parties
            .get(0)
            .cloned()
            .unwrap();
        let buyer = alice_parties
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

        // Create ticket offer
        let create_ticketoffer_result = create_ticketoffer(
            &mut command_service_client,
            Some(ticketwizard_token.as_str()),
            Some(alice_user),
            &package_id,
            organizer.clone(),
            buyer.clone(),
            amount,
        )
        .await;

        assert!(
            create_ticketoffer_result.is_ok(),
            "Ticketoffer creation failed: {:?}",
            create_ticketoffer_result
        );

        let ticketoffer_contract_id = create_ticketoffer_result.unwrap();

        let create_cash_result = create_cash(
            &mut command_service_client,
            Some(alice_token.as_str()),
            Some(alice_user),
            &package_id,
            issuer.clone(),
            owner.clone(),
            amount,
        )
        .await;

        assert!(create_cash_result.is_ok(), "Cash creation failed: {:?}", create_cash_result);

        let cash_contract_id = create_cash_result.unwrap();

        // Accept ticket offer
        let accept_result = exercise_accept(
            &mut command_service_client,
            Some(alice_token.as_str()),
            Some(alice_user),
            &package_id,
            ticketoffer_contract_id,
            cash_contract_id,
            buyer.clone(),
        )
        .await;

        assert!(accept_result.is_ok(), "Accept ticket offer failed: {:?}", accept_result);

        Ok(())
    }
}