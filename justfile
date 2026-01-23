# Run ledger-explorer with Keycloak authentication
run-ledger-explorer:
    cargo run -p ledger-explorer -- sync --use-keycloak

# Stop ledger-explorer
stop-ledger-explorer:
    pkill -f ledger-explorer || true
