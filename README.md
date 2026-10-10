# Client Toolbox for Canton

## Overview

A Rust workspace for interacting with Canton/Daml ledgers: streaming events, generating Rust types from Daml models, syncing ledger state into Neo4j, and visualizing the event graph.

Why Rust? Because it's not an officially supported client language, and building from scratch is the best way to understand the Ledger API and `.dalf` encoding.

## Prerequisites

- **Rust** (stable) with Cargo
- **Daml SDK** `3.4.8+` with the `dpm` CLI (used by `sandbox-init`, the `just` recipes and the integration tests)
- **[protoc](https://protobuf.dev/installation/)** (Protocol Buffer Compiler), needed to build `ledger-api` and `codegen`
- **Docker Desktop** or **Neo4j Desktop** (for ledger-explorer and ledger-graph-ui)
- **[just](https://github.com/casey/just)** command runner (optional, for convenience recipes)
- **[Dioxus CLI](https://dioxuslabs.com/learn/0.7/getting_started/)** `dx` 0.7 (only for ledger-graph-ui)

Build everything with:

```
cargo build --workspace
```

The checked-in `.vscode/settings.json` points rust-analyzer at protoc installed by Homebrew on Apple Silicon (`/opt/homebrew/bin/protoc`). Change the `PROTOC` path there if yours lives elsewhere (`which protoc`).

## Workspace Crates

| Crate | Type | Description |
|-------|------|-------------|
| **ledger-api** | lib | Generated Protobuf/gRPC bindings for the Daml Ledger API v2 |
| **client** | bin/lib | CLI and library wrapping Ledger API operations (streaming, JWT, party management) |
| **codegen** | bin | Generates Rust structs from DAR packages, mirroring Daml template payloads and choice records |
| **daml-type-rep** | lib | Type representations for Daml values (built-in types, numeric scaling, template IDs) |
| **derive-lapi-access** | proc-macro | Derive macro implementing the `LapiAccess` trait for gRPC type conversions |
| **ledger-explorer** | bin | Streams Canton events into Neo4j with resilient sync, batching, and configurable argument flattening |
| **ledger-graph-ui** | bin | Dioxus fullstack web UI for visualizing the Neo4j event graph |
| **submit** | lib | Library for submitting Daml contracts and exercising choices via gRPC |
| **sandbox-init** | bin | CLI tool to start a Daml sandbox and run initialization scripts from DAR files |
| **wallet** | bin | Placeholder for wallet functionality (not yet implemented) |
| **test** (`daml-model-rep-examples`) | lib | Integration tests for the `LapiAccess` trait paired with the Daml examples in `_daml/` |

### client CLI

```
cargo run -p client -- <subcommand> <params>
```

| Subcommand | Description |
|------------|-------------|
| `get-ledger-end` | Get the ledger end offset |
| `fake-access-token` | Create a fake access token for Sandbox |
| `stream-updates` | Stream ledger updates for a party |
| `stream-transactions` | Stream transactions for a party |
| `parties` | List parties, optionally filtered by substring |

Run `cargo run -p client -- <subcommand> --help` for parameter details.

### ledger-explorer

Streams the Canton event graph into Neo4j with:
- Resilient sync with automatic reconnection and exponential backoff
- Keycloak OAuth2 authentication (client credentials or password grant)
- Configurable argument flattening into dot-separated Neo4j properties
- ACS (Active Contract Set) bootstrapping
- Configurable batching, flush timeouts, and idle detection
- Multi-profile config support (local, devnet, mainnet)

Configuration goes in `ledger-explorer/config/config.toml` (gitignored). Start from the example:

```
cp ledger-explorer/config/config.toml.example ledger-explorer/config/config.toml
```

See [ledger-explorer/README.md](ledger-explorer/README.md) for the graph schema, CLI flags and example queries.

See also: [A Daml ledger tells a story](https://discuss.daml.com/t/blog-post-a-daml-ledger-tells-a-story-it-s-time-to-show-it-to-everyone/6734).

### ledger-graph-ui

A Dioxus fullstack app that reads the Neo4j graph written by ledger-explorer. It looks for `config/config.toml` in its own folder, falling back to `../ledger-explorer/config/config.toml`, and uses its `[neo4j]` section. Run it with the Dioxus CLI:

```
cd ledger-graph-ui
dx serve
```

### codegen

Generates Rust structs from DAR packages:

```
just codegen output.rs path/to/file.dar
```

Supports nested structs, modules, variants, enums, type aliases, and generics.

## Running

Common tasks are available as `just` recipes:

```
just                              # list all recipes
just explorer-run                 # sync with Keycloak auth (release mode)
just explorer-fresh               # fresh start: clear Neo4j + reload ACS
just explorer-sandbox             # sync against local sandbox (fake JWT)
just explorer-sandbox-fresh       # fresh start against local sandbox
just explorer-stop                # stop the explorer process
just codegen out.rs file.dar      # generate Rust types from a DAR
just test-nested                  # run nested-test integration test
just sandbox-init-ticketoffer     # init sandbox with ticketoffer example
```

## Daml Examples

Test Daml models live in the `_daml/` folder. Each is a multi-package project with a `main` package (templates) and a `test` package (a Daml script that allocates parties and creates contracts).

The `sandbox-init-*` recipes build a project with `dpm build --all`, start `dpm sandbox` with the main DAR on `localhost:6865`, and run the setup script, for example:

```
just sandbox-init-ticketoffer
```

To list the allocated party IDs (the local sandbox runs without authentication, so any token value works):

```
cargo run -p client -- parties --url http://localhost:6865 --access-token sandbox
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.

The vendored Daml Ledger API and Daml-LF `.proto` files are © Digital Asset and licensed under Apache-2.0. The vendored Google protobuf definitions keep their original license headers.
