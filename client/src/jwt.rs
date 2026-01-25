use serde_json::json;
use base64::{engine::general_purpose, Engine as _};
use chrono::{Utc, Duration};
use anyhow::{Result, Context};
use serde::Deserialize;
use tracing::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration as StdDuration, Instant};

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

/// Authentication method for Keycloak
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "grant_type", rename_all = "snake_case")]
pub enum KeycloakAuthMethod {
    /// OAuth2 Client Credentials flow (service accounts)
    ClientCredentials {
        client_secret: String,
    },
    /// OAuth2 Resource Owner Password Credentials flow (user authentication)
    Password {
        username: String,
        password: String,
        #[serde(default)]
        client_secret: Option<String>,
    },
}

/// Configuration for Keycloak OAuth2 authentication
#[derive(Debug, Clone, Deserialize)]
pub struct KeycloakConfig {
    pub client_id: String,
    pub token_endpoint: String,
    #[serde(flatten)]
    pub auth_method: KeycloakAuthMethod,
}

/// Response from Keycloak token endpoint
#[derive(Debug, Deserialize)]
struct KeycloakTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    expires_in: Option<u64>,
    #[allow(dead_code)]
    token_type: Option<String>,
}

/// Fetches a real JWT token from Keycloak using OAuth2 authentication.
///
/// Supports both client credentials and password grant flows.
/// This is suitable for production environments where you need a properly signed JWT
/// from a Keycloak server.
///
/// # Arguments
/// * `config` - Keycloak configuration containing authentication details
///
/// # Returns
/// * `Result<String>` - The access token on success
pub async fn keycloak_jwt(config: &KeycloakConfig) -> Result<String> {
    debug!(
        token_endpoint = %config.token_endpoint,
        client_id = %config.client_id,
        "Requesting JWT token from Keycloak"
    );

    let client = reqwest::Client::new();

    // Build parameters based on authentication method
    let mut params = vec![("client_id", config.client_id.clone())];

    match &config.auth_method {
        KeycloakAuthMethod::ClientCredentials { client_secret } => {
            params.push(("grant_type", "client_credentials".to_string()));
            params.push(("client_secret", client_secret.clone()));
        }
        KeycloakAuthMethod::Password { username, password, client_secret } => {
            params.push(("grant_type", "password".to_string()));
            params.push(("username", username.clone()));
            params.push(("password", password.clone()));
            if let Some(secret) = client_secret {
                params.push(("client_secret", secret.clone()));
            }
        }
    }

    let response = client
        .post(&config.token_endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .with_context(|| format!("Failed to send request to Keycloak at {}", config.token_endpoint))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "Unable to read response body".to_string());
        anyhow::bail!(
            "Keycloak token request failed with status {}: {}",
            status,
            body
        );
    }

    let token_response: KeycloakTokenResponse = response
        .json()
        .await
        .context("Failed to parse Keycloak token response")?;

    info!("Successfully obtained JWT token from Keycloak");

    Ok(token_response.access_token)
}

/// Keycloak token response with expiry information
#[derive(Debug, Deserialize)]
struct KeycloakTokenResponseWithExpiry {
    access_token: String,
    expires_in: u64,
    #[allow(dead_code)]
    token_type: Option<String>,
}

/// Fetches a JWT token from Keycloak and returns both the token and its expiry duration.
///
/// Supports both client credentials and password grant flows.
pub async fn keycloak_jwt_with_expiry(config: &KeycloakConfig) -> Result<(String, u64)> {
    debug!(
        token_endpoint = %config.token_endpoint,
        client_id = %config.client_id,
        "Requesting JWT token from Keycloak"
    );

    let client = reqwest::Client::new();

    // Build parameters based on authentication method
    let mut params = vec![("client_id", config.client_id.clone())];

    match &config.auth_method {
        KeycloakAuthMethod::ClientCredentials { client_secret } => {
            params.push(("grant_type", "client_credentials".to_string()));
            params.push(("client_secret", client_secret.clone()));
        }
        KeycloakAuthMethod::Password { username, password, client_secret } => {
            params.push(("grant_type", "password".to_string()));
            params.push(("username", username.clone()));
            params.push(("password", password.clone()));
            if let Some(secret) = client_secret {
                params.push(("client_secret", secret.clone()));
            }
        }
    }

    let response = client
        .post(&config.token_endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .with_context(|| format!("Failed to send request to Keycloak at {}", config.token_endpoint))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "Unable to read response body".to_string());
        anyhow::bail!(
            "Keycloak token request failed with status {}: {}",
            status,
            body
        );
    }

    let token_response: KeycloakTokenResponseWithExpiry = response
        .json()
        .await
        .context("Failed to parse Keycloak token response")?;

    info!("Successfully obtained JWT token from Keycloak (expires in {} seconds)", token_response.expires_in);

    // Log the decoded JWT claims for debugging
    log_jwt_claims(&token_response.access_token);

    Ok((token_response.access_token, token_response.expires_in))
}

/// Decodes and logs the claims from a JWT token for debugging purposes.
/// Only logs the payload (middle part), not the signature.
pub fn log_jwt_claims(token: &str) {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        warn!("Invalid JWT format - cannot decode claims");
        return;
    }

    // Decode the payload (second part)
    match general_purpose::URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(decoded) => {
            match String::from_utf8(decoded) {
                Ok(json_str) => {
                    match serde_json::from_str::<serde_json::Value>(&json_str) {
                        Ok(claims) => {
                            info!("JWT claims: {}", serde_json::to_string_pretty(&claims).unwrap_or_else(|_| json_str.clone()));

                            // Log specific important claims
                            if let Some(sub) = claims.get("sub") {
                                info!("  sub (subject): {}", sub);
                            }
                            if let Some(aud) = claims.get("aud") {
                                info!("  aud (audience): {}", aud);
                            }
                            if let Some(scope) = claims.get("scope") {
                                info!("  scope: {}", scope);
                            }
                            if let Some(act_as) = claims.get("actAs") {
                                info!("  actAs (authorized parties): {}", act_as);
                            }
                            if let Some(read_as) = claims.get("readAs") {
                                info!("  readAs (read-only parties): {}", read_as);
                            }
                            // Canton-specific claims
                            if let Some(party) = claims.get("party") {
                                info!("  party: {}", party);
                            }
                            if let Some(parties) = claims.get("parties") {
                                info!("  parties: {}", parties);
                            }
                        }
                        Err(e) => {
                            debug!("JWT payload (raw): {}", json_str);
                            warn!("Failed to parse JWT claims as JSON: {}", e);
                        }
                    }
                }
                Err(e) => warn!("JWT payload is not valid UTF-8: {}", e),
            }
        }
        Err(e) => {
            // Try with standard base64 padding
            let padded = match parts[1].len() % 4 {
                2 => format!("{}==", parts[1]),
                3 => format!("{}=", parts[1]),
                _ => parts[1].to_string(),
            };
            match general_purpose::STANDARD.decode(&padded) {
                Ok(decoded) => {
                    if let Ok(json_str) = String::from_utf8(decoded) {
                        info!("JWT payload: {}", json_str);
                    }
                }
                Err(_) => warn!("Failed to decode JWT payload: {}", e),
            }
        }
    }
}

/// Token source configuration - determines how tokens are obtained
#[derive(Clone)]
pub enum TokenSource {
    /// Use a static token (e.g., provided via CLI)
    Static(String),
    /// Use Keycloak for token management with automatic renewal
    Keycloak(KeycloakConfig),
    /// Use fake JWT for local development
    FakeJwt(String), // user_id
}

/// Internal state for the token manager
struct TokenState {
    token: String,
    obtained_at: Instant,
    expires_in_secs: u64,
}

/// Manages JWT tokens with proactive renewal before expiry.
///
/// The token manager will automatically refresh the token when 80% of its
/// lifetime has elapsed, ensuring uninterrupted service.
pub struct TokenManager {
    source: TokenSource,
    state: Arc<RwLock<Option<TokenState>>>,
    /// Renewal threshold as a fraction (0.8 = renew when 80% of lifetime elapsed)
    renewal_threshold: f64,
}

impl TokenManager {
    /// Creates a new TokenManager with the given token source.
    pub fn new(source: TokenSource) -> Self {
        Self {
            source,
            state: Arc::new(RwLock::new(None)),
            renewal_threshold: 0.8,
        }
    }

    /// Creates a TokenManager with a custom renewal threshold.
    /// threshold should be between 0.0 and 1.0 (e.g., 0.8 = renew at 80% of lifetime)
    pub fn with_renewal_threshold(source: TokenSource, threshold: f64) -> Self {
        Self {
            source,
            state: Arc::new(RwLock::new(None)),
            renewal_threshold: threshold.clamp(0.1, 0.95),
        }
    }

    /// Gets a valid token, refreshing if necessary.
    /// This is the primary method to use when making API calls.
    pub async fn get_token(&self) -> Result<String> {
        // First check if we have a valid token
        {
            let state = self.state.read().await;
            if let Some(ref token_state) = *state {
                if !self.should_refresh(token_state) {
                    return Ok(token_state.token.clone());
                }
            }
        }

        // Need to refresh - acquire write lock
        self.refresh_token().await
    }

    /// Checks if the token should be refreshed based on the renewal threshold.
    fn should_refresh(&self, state: &TokenState) -> bool {
        let elapsed = state.obtained_at.elapsed();
        let lifetime = StdDuration::from_secs(state.expires_in_secs);
        let threshold_duration = lifetime.mul_f64(self.renewal_threshold);

        if elapsed >= threshold_duration {
            debug!(
                elapsed_secs = elapsed.as_secs(),
                threshold_secs = threshold_duration.as_secs(),
                expires_in_secs = state.expires_in_secs,
                "Token needs refresh"
            );
            true
        } else {
            false
        }
    }

    /// Forces a token refresh regardless of expiry.
    pub async fn refresh_token(&self) -> Result<String> {
        let mut state = self.state.write().await;

        // Double-check after acquiring write lock
        if let Some(ref token_state) = *state {
            if !self.should_refresh(token_state) {
                return Ok(token_state.token.clone());
            }
        }

        info!("Refreshing JWT token");

        let (token, expires_in_secs) = match &self.source {
            TokenSource::Static(t) => {
                // Static tokens don't expire (or we don't know when)
                (t.clone(), u64::MAX)
            }
            TokenSource::Keycloak(config) => {
                keycloak_jwt_with_expiry(config).await?
            }
            TokenSource::FakeJwt(user_id) => {
                // Fake JWTs are set to expire in 24 hours
                let token = fake_jwt_for_user(user_id);
                (token, 24 * 60 * 60)
            }
        };

        *state = Some(TokenState {
            token: token.clone(),
            obtained_at: Instant::now(),
            expires_in_secs,
        });

        info!("JWT token refreshed (expires in {} seconds)", expires_in_secs);
        Ok(token)
    }

    /// Returns the time until the next refresh is needed, if known.
    pub async fn time_until_refresh(&self) -> Option<StdDuration> {
        let state = self.state.read().await;
        state.as_ref().map(|s| {
            let lifetime = StdDuration::from_secs(s.expires_in_secs);
            let threshold_duration = lifetime.mul_f64(self.renewal_threshold);
            let elapsed = s.obtained_at.elapsed();
            threshold_duration.saturating_sub(elapsed)
        })
    }

    /// Starts a background task that proactively refreshes the token.
    /// Returns a handle that can be used to abort the task.
    pub fn start_background_refresh(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                // Get the time until next refresh
                let sleep_duration = match self.time_until_refresh().await {
                    Some(d) if d > StdDuration::ZERO => d,
                    _ => StdDuration::from_secs(60), // Check every minute if no valid token
                };

                debug!(
                    sleep_secs = sleep_duration.as_secs(),
                    "Background token refresh sleeping"
                );

                tokio::time::sleep(sleep_duration).await;

                // Refresh the token
                match self.refresh_token().await {
                    Ok(_) => info!("Background token refresh successful"),
                    Err(e) => warn!("Background token refresh failed: {}. Will retry.", e),
                }
            }
        })
    }
}