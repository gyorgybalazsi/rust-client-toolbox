# List available recipes
default:
    @just --list

# Run explorer with Keycloak authentication (release mode)
explorer-run:
    cargo run --release -p ledger-explorer -- sync --use-keycloak 2>&1 | tee sync.log

# Fresh start: clear Neo4j, load current ACS, and stream from ledger end
explorer-fresh:
    cargo run --release -p ledger-explorer -- sync --use-keycloak --fresh 2>&1 | tee sync.log

# Run explorer against local sandbox (no Keycloak, fake JWT)
explorer-sandbox:
    cargo run --release -p ledger-explorer -- sync --profile local 2>&1 | tee sync.log

# Fresh start against local sandbox (no Keycloak)
explorer-sandbox-fresh:
    cargo run --release -p ledger-explorer -- sync --profile local --fresh 2>&1 | tee sync.log

# Stop explorer
explorer-stop:
    pkill -f ledger-explorer || true
