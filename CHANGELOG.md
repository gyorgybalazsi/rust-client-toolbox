# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Flatten create/choice arguments into dot-separated Neo4j node properties
  (e.g., `create_arg.person.homeAddress.city = "Zurich"`) for direct querying
- Configurable `[storage]` section in config.toml: `flatten_arguments`,
  `flatten_max_depth`, `store_arguments_json`
- Auto-discover parties from ledger when none configured (`ListKnownParties`)
- `store_arguments_json` flag to optionally store raw JSON blobs alongside
  flattened properties (default: false)

### Changed
- Renamed justfile recipes: `explorer-run`, `explorer-sandbox`,
  `explorer-fresh`, `explorer-sandbox-fresh`, `explorer-stop`
- `PrintCypher` and `Benchmark` commands now respect storage config instead of
  using hardcoded flatten settings
