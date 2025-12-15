#!/bin/bash
set -e

# Build Daml packages
(cd _daml/daml-asset && dpm build --all)

# Run sandbox-init
cargo run --bin sandbox-init \
  -- --dar _daml/daml-asset/main/.daml/dist/daml-asset-0.0.1.dar \
  --init-dar _daml/daml-asset/test/.daml/dist/daml-asset-test-0.0.1.dar \
  --init-script-name "Test:setup"