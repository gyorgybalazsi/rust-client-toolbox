# Connecting to Canton Validators

This guide explains how to connect the ledger-explorer to Canton validators (DevNet or MainNet) using Keycloak JWT authentication.

## Prerequisites

1. **kubectl** configured with access to the Canton cluster
2. **Port-forward** to the validator participant
3. **Keycloak credentials** for the target network
4. **Neo4j** running locally (or update config with your Neo4j connection)

## Setup Steps

### 1. Port-Forward the Validator

Create a port-forward from your local machine to the validator's Ledger API port:

#### DevNet
```bash
kubectl port-forward svc/participant-ibtc-devnet-1 -n catalyst-canton 5001:5001
```

#### MainNet
```bash
kubectl port-forward svc/participant-ibtc-mainnet -n catalyst-canton 5001:5001
```

Keep this terminal running while the ledger-explorer is active.

### 2. Configure the Ledger Explorer

Edit `ledger-explorer/config/config.toml` with the appropriate settings.

#### DevNet Configuration

```toml
[logging]
level = "info"

[neo4j]
uri = "neo4j://127.0.0.1:7687"
user = "neo4j"
password = "password"

[ledger]
fake_jwt_user = "alice"
parties = ["iBTC-validator-1::1220fa8543db6c66fe3a55b1f180c8dfc7f876265c76684fbc1d35d89e02c8aafe8e"]
url = "http://localhost:5001"
begin_offset = 23102  # Adjust based on pruned offset

[keycloak]
client_id = "your-devnet-client-id"
client_secret = "your-devnet-client-secret"
token_endpoint = "https://keycloak.dev.example.com/auth/realms/your-realm/protocol/openid-connect/token"
```

#### MainNet Configuration

```toml
[logging]
level = "info"

[neo4j]
uri = "neo4j://127.0.0.1:7687"
user = "neo4j"
password = "password"

[ledger]
fake_jwt_user = "alice"
parties = ["cbtc-network::12205af3b949a04776fc48cdcc05a060f6bda2e470632935f375d1049a8546a3b262"]
url = "http://localhost:5001"
begin_offset = 579155  # Adjust based on pruned offset

[keycloak]
client_id = "your-mainnet-client-id"
client_secret = "your-mainnet-client-secret"
token_endpoint = "https://keycloak.example.com/auth/realms/your-realm/protocol/openid-connect/token"
```

### 3. Run the Ledger Explorer

```bash
cargo run -p ledger-explorer -- sync --use-keycloak
```

The `--use-keycloak` flag tells the explorer to obtain a real JWT token from Keycloak instead of generating a fake one.

## Understanding the Configuration

### Party IDs

The `parties` field specifies which party's transactions to sync. The party ID must match what the Keycloak service account has access to:

- **DevNet**: `iBTC-validator-1::1220fa8543db6c66fe3a55b1f180c8dfc7f876265c76684fbc1d35d89e02c8aafe8e`
- **MainNet**: `cbtc-network::12205af3b949a04776fc48cdcc05a060f6bda2e470632935f375d1049a8546a3b262`

### Begin Offset

The `begin_offset` determines where to start streaming transactions from. This must be set to a value that hasn't been pruned by the participant:

- If you see an error like `PARTICIPANT_PRUNED_DATA_ACCESSED: ... precedes pruned offset 579155`, update the `begin_offset` to that value.
- Set to `0` to start from the beginning (if data hasn't been pruned).

### Keycloak Configuration

The Keycloak section contains OAuth2 client credentials:

- `client_id`: The service account client ID
- `client_secret`: The client secret for authentication
- `token_endpoint`: The Keycloak OAuth2 token endpoint URL

## Troubleshooting

### Permission Denied Errors

If you see `PERMISSION_DENIED: Claims do not authorize to read data for party`, verify:

1. The party ID in the config matches what the Keycloak service account has access to
2. You're using the correct Keycloak credentials for the target network

### Pruned Data Errors

If you see `PARTICIPANT_PRUNED_DATA_ACCESSED`, update the `begin_offset` in your config to the value mentioned in the error message.

### Connection Refused

If you see connection errors:

1. Verify the port-forward is running: `ps aux | grep port-forward`
2. Check that the validator is reachable: `curl -v http://localhost:5001`
3. Ensure Neo4j is running: `curl http://localhost:7474`

## Monitoring Progress

Check how many nodes have been loaded into Neo4j:

```bash
curl -s -u neo4j:password -H "Content-Type: application/json" \
  -d '{"statements":[{"statement":"MATCH (n) RETURN labels(n) as type, count(*) as count ORDER BY count DESC"}]}' \
  http://localhost:7474/db/neo4j/tx/commit | jq -r '.results[0].data[] | "\(.row[0]): \(.row[1])"'
```

## Authentication Flow

When using `--use-keycloak`, the ledger-explorer:

1. Sends a client credentials grant request to Keycloak
2. Receives a JWT access token
3. Adds the token to the Ledger API requests as `Authorization: Bearer <token>`
4. The Canton participant validates the token and authorizes access

The token includes claims that identify the service account and its permissions:
- `sub`: Service account identifier (e.g., `service-account-cbtc-network-reader`)
- `client_id`: The Keycloak client ID
- `scope`: Includes `daml_ledger_api` for ledger access

## Security Notes

- Keep the `config.toml` file secure as it contains sensitive credentials
- The `config.toml` is gitignored by default to prevent accidental commits
- Use the provided `config.toml.example` as a template for new setups
