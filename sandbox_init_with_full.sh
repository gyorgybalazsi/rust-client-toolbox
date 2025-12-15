#!/bin/bash
set -e

# Build Daml packages
(cd _daml/full && dpm build --all)

# Run sandbox-init
cargo run --bin sandbox-init \
  -- --dar _daml/full/main/.daml/dist/full-0.0.1.dar \
  --init-dar _daml/full/test/.daml/dist/full-test-0.0.1.dar \
  --init-script-name "Test:setup"
