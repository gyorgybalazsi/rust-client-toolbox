#!/bin/bash
set -e

# Build Daml packages
(cd _daml/daml-optional && dpm build --all)

# Run sandbox-init
cargo run --bin sandbox-init \
  -- --dar _daml/daml-optional/main/.daml/dist/daml-optional-0.0.1.dar \
  --init-dar _daml/daml-optional/test/.daml/dist/daml-optional-test-0.0.1.dar \
  --init-script-name "Test:setup"
