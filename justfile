# Run ledger-explorer with Keycloak authentication (release mode for performance)
run-ledger-explorer:
    cargo run --release -p ledger-explorer -- sync --use-keycloak

# Stop ledger-explorer
stop-ledger-explorer:
    pkill -f ledger-explorer || true
