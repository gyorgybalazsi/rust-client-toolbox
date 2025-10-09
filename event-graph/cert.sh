mkdir -p ./neo4j/certificates/neo4j
openssl req -x509 -newkey rsa:4096 -keyout ./neo4j/certificates/neo4j/neo4j.key -out ./neo4j/certificates/neo4j/neo4j.crt -days 365 -nodes -subj "/CN=localhost"