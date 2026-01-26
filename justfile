# Run ledger-explorer with Keycloak authentication (release mode for performance)
run-ledger-explorer:
    cargo run --release -p ledger-explorer -- sync --use-keycloak

# Fresh start: clear Neo4j, load current ACS, and stream from ledger end
fresh-start:
    cargo run --release -p ledger-explorer -- sync --use-keycloak --fresh

# Stop ledger-explorer
stop-ledger-explorer:
    pkill -f ledger-explorer || true
