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

# Generate Rust types from DAR file(s)
# Usage: just codegen output.rs path/to/file.dar [path/to/other.dar ...]
codegen output +dars:
    cargo run -p codegen -- --dar {{dars}} -o {{output}}

# --- Integration tests ---

# Run the nested-test integration test (starts sandbox, creates contract, exercises choice)
test-nested:
    cd _daml/daml-nested-test && dpm build --all
    cargo test -p daml-model-rep-examples nested_test -- --nocapture

# --- Sandbox init recipes ---
# Each builds the DAML packages and runs sandbox-init

sandbox-init-asset:
    cd _daml/daml-asset && dpm build --all
    cargo run --bin sandbox-init -- \
        --dar _daml/daml-asset/main/.daml/dist/daml-asset-0.0.1.dar \
        --init-dar _daml/daml-asset/test/.daml/dist/daml-asset-test-0.0.1.dar \
        --init-script-name "Test:setup"

sandbox-init-interface:
    cd _daml/daml-interface-example && dpm build --all
    cargo run --bin sandbox-init -- \
        --dar _daml/daml-interface-example/main/.daml/dist/daml-interface-example-main-1.0.0.dar \
        --init-dar _daml/daml-interface-example/test/.daml/dist/daml-interface-example-test-1.0.0.dar \
        --init-script-name "Test:test"

sandbox-init-optional:
    cd _daml/daml-optional && dpm build --all
    cargo run --bin sandbox-init -- \
        --dar _daml/daml-optional/main/.daml/dist/daml-optional-0.0.1.dar \
        --init-dar _daml/daml-optional/test/.daml/dist/daml-optional-test-0.0.1.dar \
        --init-script-name "Test:setup"

sandbox-init-ticketoffer:
    cd _daml/daml-ticketoffer && dpm build --all
    cargo run --bin sandbox-init -- \
        --dar _daml/daml-ticketoffer/main/.daml/dist/daml-ticketoffer-0.0.1.dar \
        --init-dar _daml/daml-ticketoffer/test/.daml/dist/daml-ticketoffer-test-0.0.1.dar \
        --init-script-name "Test:setup"

sandbox-init-ticketoffer-explicit-disclosure:
    cd _daml/daml-ticketoffer-explicit-disclosure && dpm build --all
    cargo run --bin sandbox-init -- \
        --dar _daml/daml-ticketoffer-explicit-disclosure/main/.daml/dist/daml-ticketoffer-explicit-disclosure-0.0.1.dar \
        --init-dar _daml/daml-ticketoffer-explicit-disclosure/test/.daml/dist/daml-ticketoffer-explicit-disclosure-test-0.0.1.dar \
        --init-script-name "Setup:setup"

sandbox-init-full:
    cd _daml/full && dpm build --all
    cargo run --bin sandbox-init -- \
        --dar _daml/full/main/.daml/dist/full-0.0.1.dar \
        --init-dar _daml/full/test/.daml/dist/full-test-0.0.1.dar \
        --init-script-name "Test:setup"
