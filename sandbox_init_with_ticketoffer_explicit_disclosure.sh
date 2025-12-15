#!/bin/bash
set -e

# Build Daml packages
(cd _daml/daml-ticketoffer-explicit-disclosure && dpm build --all)

# Run sandbox-init
cargo run --bin sandbox-init \
  -- --dar _daml/daml-ticketoffer-explicit-disclosure/main/.daml/dist/daml-ticketoffer-explicit-disclosure-0.0.1.dar \
  --init-dar _daml/daml-ticketoffer-explicit-disclosure/test/.daml/dist/daml-ticketoffer-explicit-disclosure-test-0.0.1.dar \
  --init-script-name "Setup:setup"
