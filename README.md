# Client toolbox for Canton

## Overview

I decided to roll my own client toolbox so that I understand more the LAPI and the `.dalf` encoding of Daml models. 

Why Rust?

Because rust is cool, and it's not an officially supported client language. 

In early stage.

Main features planned:

- Ledger viewer/ACS source, based on event graph handling and visualization (see my blog post: [A Daml ledger tells a story — it’s time to show it to everyone](https://discuss.daml.com/t/blog-post-a-daml-ledger-tells-a-story-it-s-time-to-show-it-to-everyone/6734))
- Rust codegen (partly inspired by [Rust Bindings for Daml](https://github.com/fujiapple852/rust-daml-bindings))
- etc.

## Prerequisits

DAMl SDK, version `3.4.0-rc2`

[The Protocol Buffer Compiler (protoc)](https://protobuf.dev/installation/)

The `.vscode/settings.json` file should contain the following:

```
{
    "rust-analyzer.server.extraEnv": {
        "PROTOC": "/usr/local/bin/protoc"
    }
}
```

Docker Desktop or Neo4J desktop to run the ledger explorer.

## Daml examples

Daml examples for testing can be found in the `_daml` folder. 

All test examples contain a Daml script which allocates some parties when started with the `Daml start` command.

(Please note that including Daml script in Daml model packages is strongly discouraged in production.)

One easy way to retrieve the Daml parties after starting Sandbox is the following:

- Start the Canton console against Sandbox with the `daml canton-console` command.
- Print the party IDs with the following Scala command: `sandbox.parties.list().map(_.party.toProtoPrimitive)`

## Crates

### client

The `client` command is a wrapper around some fetures implemented in this project. 

TODO: add more features.

Either build it and run, other just run with `cargo run -p client -- <subcommand> <params>`.

The subcommands are:

| Subcommand | Description | Params |
|------------|-------------|--------|
| get-ledger-end | Get the ledger end | --url, --access-token |
| fake-access-token | Create fake access token for Sandbox | --url, --party |
| stream-updates | Stream ledger updates for a party | --url, --access-token, --party, --begin-exclusive, --end-inclusive (optional) |
| stream-transactions | Stream transactions for a party | --url, --access-token, --party, --begin-exclusive, --end-inclusive (optional) |
| parties | Get parties, optionally filtered by a substring | --url, --access-token, --filter (substring, optional) |

The subcommand params can be get with the comand `cargo run -p client -- <subcommand> --help`.

### codegen

Contains code to generate Rust structs from a DAR package, mirroring the Daml template payload and choice input records. 

Example: the `codegen/generated/ticketoffer_structs.rs` file contains Rust structs generated from the `_daml/daml-ticketoffer` package.

TODO: implement a module structure in the generated Rust code, mirroring the input Daml code module structure.

### derive-lapi-access

Contains a derive macro which implements the `LapiAccess` trait.

The `LapiAccess` trait contains type conversion functions for gRPC ledger API access.

TODO: cover all Daml types.

### test

Contains tests for the `LapiAccess` trait, paired with the Daml examples in the `_daml` folder.

### ledger-explorer

An app which loads the event graph representation of a Canton ledger into a Neo4J graph DB instance. 

The code to run the ledger explorer as contained in the `run-ledger-explorer` folder. 

See the `run-ledger-explorer/README.md` for instructions. 

TODO: 

1. Logging
2. Enhance payload representation using codegen
3. Optionally include raw bytes of the original ledger content

