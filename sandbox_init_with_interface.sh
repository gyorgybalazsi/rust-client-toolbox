#!/bin/bash
set -e

# Build Daml packages
(cd _daml/daml-interface-example && dpm build --all)

# Run sandbox-init
cargo run --bin sandbox-init \
  -- --dar _daml/daml-interface-example/main/.daml/dist/daml-interface-example-main-1.0.0.dar \
  --init-dar _daml/daml-interface-example/test/.daml/dist/daml-interface-example-test-1.0.0.dar \
  --init-script-name "Test:test"
