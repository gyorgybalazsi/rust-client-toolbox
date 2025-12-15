#!/bin/bash
set -e

# Build Daml packages
(cd _daml/daml-ticketoffer && dpm build --all)

# Run sandbox-init
cargo run --bin sandbox-init \
  -- --dar _daml/daml-ticketoffer/main/.daml/dist/daml-ticketoffer-0.0.1.dar \
  --init-dar _daml/daml-ticketoffer/test/.daml/dist/daml-ticketoffer-test-0.0.1.dar \
  --init-script-name "Test:setup"
