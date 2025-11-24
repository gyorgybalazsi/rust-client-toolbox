# Run the ledger explorer

## Supported Daml SDK

This app was tested with sdk-version 3.4.8

The ledger sync uses ledger API v2.

## Run Canton

The default settings suppose that you run Sandbox.

You can connect to Canton 3.4 in any other way (Docker, Canton Network validator node with port forward, etc.).

If you do not use Sandbox, adjust the settings accordingly.

The app always requires a JWT token.

## Run Neo4j

Download [Neo4J desktop](https://neo4j.com/download/).

You can import and use the saved cypher query collection stored in the current folder. 

## Run the ledger explorer

Adjust the config params in the `config/config.toml` file.

Run the ledger sync with the `run_ledger_explorer.sh` script in the project root folder. 

## Learn Cypher

You can explore the event graph with Cypher queries. 

The desktop app contains a Cypher tutorial. 

