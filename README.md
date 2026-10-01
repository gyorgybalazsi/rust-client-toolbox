# Client Toolbox for Canton

## Overview

A Rust workspace for interacting with Canton/Daml ledgers: streaming events, generating Rust types from Daml models, syncing ledger state into Neo4j, and visualizing the event graph.

Why Rust? Because it's not an officially supported client language, and building from scratch is the best way to understand the Ledger API and `.dalf` encoding.

## Prerequisites

- **Daml SDK** `3.4.8+`
- **[protoc](https://protobuf.dev/installation/)** (Protocol Buffer Compiler)
- **Docker Desktop** or **Neo4j Desktop** (for ledger-explorer and ledger-graph-ui)
- **[just](https://github.com/casey/just)** command runner (optional, for convenience recipes)

Add protoc to rust-analyzer in `.vscode/settings.json`:

```json
{
    "rust-analyzer.server.extraEnv": {
        "PROTOC": "/usr/local/bin/protoc"
    }
}
```

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
| **wallet** | lib | Placeholder for wallet functionality (not yet implemented) |
| **test** | lib | Integration tests for the `LapiAccess` trait paired with Daml examples |

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

Configuration goes in `ledger-explorer/config/config.toml` (gitignored).

See also: [A Daml ledger tells a story](https://discuss.daml.com/t/blog-post-a-daml-ledger-tells-a-story-it-s-time-to-show-it-to-everyone/6734).

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

Test Daml models live in the `_daml/` folder. Each contains a Daml script that allocates parties when started with `daml start`.

To retrieve party IDs after starting Sandbox:
1. Start the Canton console: `daml canton-console`
2. List parties: `sandbox.parties.list().map(_.party.toProtoPrimitive)`
