use serde_json::json;
use base64::{engine::general_purpose, Engine as _};
use chrono::{Utc, Duration};
use anyhow::Result;

/// Creates a fake JWT token for a given party, valid for 24 hours from creation.
/// This token is unsigned (alg: "none") and suitable for local dev/testing.
pub async fn fake_jwt(
    client: &mut UserManagementServiceClient<tonic::transport::Channel>,
    party: &str,
) -> Result<String> {
    // JWT header
    let header = json!({
        "alg": "none",
        "typ": "JWT"
    });

    // Get user id for the party
    let user_id = get_user_for_party(client, party).await?;

    // JWT payload
    let payload = json!({
        "aud": "someParticipantId",
        "sub": user_id,
        "iss": "someIdpId",
        "scope": "daml_ledger_api",
        "exp": (Utc::now() + Duration::hours(24)).timestamp()
    });

    let header_enc = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string());
    let payload_enc = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string());

    // No signature for alg "none"
    Ok(format!("{}.{}.", header_enc, payload_enc))
}

use ledger_api::com::daml::ledger::api::v2::admin::user_management_service_client::UserManagementServiceClient;
use ledger_api::com::daml::ledger::api::v2::admin::ListUsersRequest;

pub async fn get_user_for_party(
    client: &mut UserManagementServiceClient<tonic::transport::Channel>,
    party: &str,
) -> Result<Option<String>> {
    let request = tonic::Request::new(ListUsersRequest {
        page_token: String::new(),
        page_size: 100, // Adjust as needed
        identity_provider_id: String::new(), // Use the appropriate ID if needed
    });
    let response = client.list_users(request).await?.into_inner();

    for user in response.users {
        if user.primary_party.as_str() == party {
            return Ok(Some(user.id.clone()));
        }
        // Optionally, check user.parties if your model supports multiple parties per user
    }
    Ok(None)
}

pub fn fake_jwt_for_user(
    user_id: &str,
) -> String {
    // JWT header
    let header = json!({
        "alg": "none",
        "typ": "JWT"
    });

    // JWT payload
    let payload = json!({
        "aud": "someParticipantId",
        "sub": user_id,
        "iss": "someIdpId",
        "scope": "daml_ledger_api",
        "exp": (Utc::now() + Duration::hours(24)).timestamp()
    });

    let header_enc = general_purpose::URL_SAFE_NO_PAD.encode(header.to_string());
    let payload_enc = general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string());

    // No signature for alg "none"
    format!("{}.{}", header_enc, payload_enc)
}