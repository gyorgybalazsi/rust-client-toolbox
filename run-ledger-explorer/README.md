# Run the ledger explorer

## Supported Daml SDK

This app was tested with sdk-version 3.4.0-snapshot.20251006.0

The ledger sync uses ledger API v2.

## Run Canton

The default settings suppose that you run Sandbox.

You can connect to Canton 3.4 in any other way (Docker, Canton Network validator node with port forward, etc.).

If you do not use Sandbox, adjust the settings accordingly.

The app always requires a JWT token.

## Run Neo4j

The easiest way is to use [Neo4J desktop](https://neo4j.com/download/).

This also contains the [Neo4J Bloom](https://neo4j.com/product/bloom/) visualization tool.

Alternativelly, you can run Neo4J in docker, using the `docker-compose.yaml` file.

Run the `cert.sh` script to generate certs for Neo4J. This is required for using the latest browser UI.

The browser UI can be found at `localhost:7474`.

Neo4J data are stored in the `data` folder. You can reset the event graph either by deleting the folder, or with the `Clear` preset cypher query (see more about this below).

You can import and use the saved cypher query collection. 

## Run the ledger sync

Run the ledger sync with the `sync.sh` script. 

## Learn Cypher

You can explore the event graph with Cypher queries. 

The desktop app and the browser UI contains a Cypher tutorial. 

## The ledger explorer source code

You can find the Rust source code in the `ledger-explorer` crate placed in the project root.

