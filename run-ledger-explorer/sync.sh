JWT="XXX"
URL="http://localhost:6865"
START_OFFSET_EXCLUSIVE="0"
PARTY="Alice::1220ae486f6ff38f32f1d2744d98a2627907d29ed0e40c29a77763453799b814437e"
NEO4J_URI="neo4j://127.0.0.1:7687"
NEO4J_USER="neo4j"
NEO4J_PASS="supersafe"

./ledger-explorer sync \
    --access-token $JWT \
    --url $URL \
    --begin-exclusive $START_OFFSET_EXCLUSIVE \
    --party $PARTY \
    --neo4j-uri $NEO4J_URI \
    --neo4j-user $NEO4J_USER \
    --neo4j-pass $NEO4J_PASS \
    # --end-inclusive $END_OFFSET_INCLUSIVE