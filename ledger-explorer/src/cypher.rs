use chrono::DateTime;
use client::utils::{
    extract_contract_ids_from_value, extract_edges, structure_markers_from_transaction,
};
use ledger_api::v2::{GetUpdatesResponse, get_updates_response::Update, event::Event};
use neo4rs::Query;
use crate::api_record_to_json::{api_record_to_json, choice_argument_json};

/// Wrapper around neo4rs::Query that preserves the cypher string for debugging
#[derive(Clone)]
pub struct CypherQuery {
    pub cypher: String,
    pub query: Query,
}

impl std::fmt::Debug for CypherQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CypherQuery")
            .field("cypher", &self.cypher)
            .finish()
    }
}

impl CypherQuery {
    pub fn new(cypher: String) -> Self {
        Self {
            query: Query::new(cypher.clone()),
            cypher,
        }
    }
}

macro_rules! cypher_query {
    ($cypher:expr, $($key:ident = $value:expr),* $(,)?) => {{
        let cypher_str = $cypher.to_string();
        let query = Query::new(cypher_str.clone())
            $(.param(stringify!($key), $value))*;
        CypherQuery { cypher: cypher_str, query }
    }};
}

/// Converts a GetUpdatesResponse directly into a Vec of Cypher statements.
/// Returns an empty vector if update is None or not a Transaction.
pub fn get_updates_response_to_cypher(response: &GetUpdatesResponse) -> Vec<CypherQuery> {
    let mut cypher_statements = Vec::new();

    let Some(update) = &response.update else {
        return cypher_statements;
    };

    let Update::Transaction(transaction) = update else {
        return cypher_statements;
    };

    // Create Transaction node with metadata
    let effective_at = transaction
        .effective_at
        .as_ref()
        .and_then(|ts| {
            let dt = DateTime::from_timestamp(ts.seconds, ts.nanos as u32);
            dt.map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        })
        .unwrap_or_default();
    let record_time = transaction
        .record_time
        .as_ref()
        .and_then(|ts| {
            let dt = DateTime::from_timestamp(ts.seconds, ts.nanos as u32);
            dt.map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        })
        .unwrap_or_default();
    let traceparent = transaction
        .trace_context
        .as_ref()
        .and_then(|tc| tc.traceparent.clone())
        .unwrap_or_default();
    let tracestate = transaction
        .trace_context
        .as_ref()
        .and_then(|tc| tc.tracestate.clone())
        .unwrap_or_default();
    let label = format!("TX@{}", transaction.offset);
    cypher_statements.push(cypher_query!(
        "CREATE (t:Transaction { \
        label: $label, \
        update_id: $update_id, \
        command_id: $command_id, \
        workflow_id: $workflow_id, \
        offset: $offset, \
        synchronizer_id: $synchronizer_id, \
        effective_at: $effective_at, \
        record_time: $record_time, \
        traceparent: $traceparent, \
        tracestate: $tracestate })",
        label = label,
        update_id = transaction.update_id.clone(),
        command_id = transaction.command_id.clone(),
        workflow_id = transaction.workflow_id.clone(),
        offset = transaction.offset,
        synchronizer_id = transaction.synchronizer_id.clone(),
        effective_at = effective_at.clone(),
        record_time = record_time.clone(),
        traceparent = traceparent.clone(),
        tracestate = tracestate.clone(),
    ));

    for event in &transaction.events {
        match &event.event {
            Some(Event::Created(created)) => {
                let label = created
                    .template_id
                    .as_ref()
                    .map(|id| format!("{}@{}", id.entity_name, created.offset))
                    .unwrap_or_else(|| format!("unknown@{}", created.offset));
                let template_name = created
                    .template_id
                    .as_ref()
                    .map(|id| format!("{}.{}", id.module_name, id.entity_name))
                    .unwrap_or_else(|| "unknown".to_string());
                let signatories_str = created
                    .signatories
                    .iter()
                    .map(|s| format!("'{}'", s))
                    .collect::<Vec<_>>()
                    .join(", ");
                let created_at = created
                    .created_at
                    .as_ref()
                    .and_then(|ts| {
                        let dt = DateTime::from_timestamp(ts.seconds, ts.nanos as u32);
                        dt.map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                    })
                    .unwrap_or_default();
                let create_arguments_json = created
                    .create_arguments
                    .as_ref()
                    .map(|args| api_record_to_json(args))
                    .map(|json| serde_json::to_string(&json).unwrap_or("null".to_string()))
                    .unwrap_or("null".to_string());
                let create_arguments = created
                    .create_arguments
                    .as_ref()
                    .map(|args| serde_json::to_string(args).unwrap_or("null".to_string()))
                    .unwrap_or("null".to_string());
                cypher_statements.push(cypher_query!(
                    "CREATE (c:Created { \
                    contract_id: $contract_id, \
                    template_name: $template_name, \
                    label: $label, \
                    signatories: $signatories, \
                    offset: $offset, \
                    node_id: $node_id, \
                    created_at: $created_at, \
                    create_arguments: $create_arguments, \
                    create_arguments_json: $create_arguments_json })",
                    contract_id = created.contract_id.clone(),
                    template_name = template_name.clone(),
                    label = label.clone(),
                    signatories = signatories_str.clone(),
                    offset = created.offset,
                    node_id = created.node_id,
                    created_at = created_at.clone(),
                    create_arguments = create_arguments,
                    create_arguments_json = create_arguments_json,
                ));
            }
            Some(Event::Exercised(exercised)) => {
                let label = format!("{}@{}", exercised.choice, exercised.offset);
                let choice_name = exercised.choice.clone();
                let acting_parties_str = exercised
                    .acting_parties
                    .iter()
                    .map(|s| format!("'{}'", s))
                    .collect::<Vec<_>>()
                    .join(", ");
                let transaction_effective_at = transaction
                    .effective_at
                    .as_ref()
                    .and_then(|ts| {
                        let dt = DateTime::from_timestamp(ts.seconds, ts.nanos as u32);
                        dt.map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                    })
                    .unwrap_or_default();
                let choice_argument_json_val = choice_argument_json(&exercised.choice_argument);
                let choice_argument_json = serde_json::to_string(&choice_argument_json_val).unwrap_or("null".to_string());
                let choice_argument = exercised
                    .choice_argument
                    .as_ref()
                    .map(|arg| serde_json::to_string(arg).unwrap_or("null".to_string()))
                    .unwrap_or("null".to_string());
                cypher_statements.push(cypher_query!(
                    "CREATE (e:Exercised { \
                    label: $label, \
                    choice_name: $choice_name, \
                    target_contract_id: $target_contract_id, \
                    acting_parties: $acting_parties, \
                    offset: $offset, \
                    node_id: $node_id, \
                    consuming: $consuming, \
                    result_contract_ids: $result_contract_ids, \
                    last_descendant_node_id: $last_descendant_node_id, \
                    transaction_effective_at: $transaction_effective_at, \
                    choice_argument: $choice_argument, \
                    choice_argument_json: $choice_argument_json })",
                    label = label.clone(),
                    choice_name = choice_name.clone(),
                    target_contract_id = exercised.contract_id.clone(),
                    acting_parties = acting_parties_str.clone(),
                    offset = exercised.offset,
                    node_id = exercised.node_id,
                    consuming = exercised.consuming,
                    result_contract_ids = serde_json::to_string(&extract_contract_ids_from_value(&exercised.exercise_result)).unwrap_or("[]".to_string()),
                    last_descendant_node_id = exercised.last_descendant_node_id,
                    transaction_effective_at = transaction_effective_at.clone(),
                    choice_argument = choice_argument,
                    choice_argument_json = choice_argument_json,
                ));
            }
            _ => {}
        }
    }

    // Add CONSEQUENCE edges based on structure_markers_from_transaction and extract_edges
    let markers = structure_markers_from_transaction(transaction);
    let edges = extract_edges(&markers);
    for (offset, parent_id, child_id) in &edges {
        cypher_statements.push(cypher_query!(
            "MATCH (parent {offset: $offset, node_id: $parent_id}), \
            (child {offset: $offset, node_id: $child_id}) \
            CREATE (parent)-[:CONSEQUENCE]->(child)",
            offset = *offset,
            parent_id = *parent_id,
            child_id = *child_id,
        ));
    }

    // Add TARGET relationships: from Exercised node to Created node with matching contract_id
    for event in &transaction.events {
        if let Some(Event::Exercised(exercised)) = &event.event {
            cypher_statements.push(cypher_query!(
                "MATCH (e:Exercised {offset: $offset, node_id: $node_id}), \
                (c:Created {contract_id: $target_contract_id}) \
                CREATE (e)-[:TARGET]->(c)",
                offset = exercised.offset,
                node_id = exercised.node_id,
                target_contract_id = exercised.contract_id.clone(),
            ));
            if exercised.consuming {
                cypher_statements.push(cypher_query!(
                    "MATCH (e:Exercised {offset: $offset, node_id: $node_id}), \
                    (c:Created {contract_id: $target_contract_id}) \
                    CREATE (e)-[:CONSUMES]->(c)",
                    offset = exercised.offset,
                    node_id = exercised.node_id,
                    target_contract_id = exercised.contract_id.clone(),
                ));
            }
        }
    }

    // Identify root-level events (those not in any edge as a child)
    let child_node_ids: std::collections::HashSet<i32> = edges.iter().map(|(_, _, child)| *child).collect();

    // Collect requester parties from root-level events and connect Transaction to root-level events
    let offset = transaction.offset;
    let mut requesting_parties: std::collections::HashSet<String> = std::collections::HashSet::new();

    for event in &transaction.events {
        if let Some(Event::Exercised(exercised)) = &event.event {
            // Root-level Exercised event: not a child of any other event
            if !child_node_ids.contains(&exercised.node_id) {
                // Collect acting parties as requesting parties
                for party in &exercised.acting_parties {
                    requesting_parties.insert(party.clone());
                }
                // Connect Transaction to this root-level Exercised event
                cypher_statements.push(cypher_query!(
                    "MATCH (t:Transaction {offset: $offset}), \
                    (e:Exercised {offset: $offset, node_id: $node_id}) \
                    CREATE (t)-[:ACTION]->(e)",
                    offset = offset,
                    node_id = exercised.node_id,
                ));
            }
        }
        if let Some(Event::Created(created)) = &event.event {
            // Root-level Created event: not a child of any other event
            if !child_node_ids.contains(&created.node_id) {
                // Collect signatories as requesting parties
                for party in &created.signatories {
                    requesting_parties.insert(party.clone());
                }
                // Connect Transaction to this root-level Created event
                cypher_statements.push(cypher_query!(
                    "MATCH (t:Transaction {offset: $offset}), \
                    (c:Created {offset: $offset, node_id: $node_id}) \
                    CREATE (t)-[:ACTION]->(c)",
                    offset = offset,
                    node_id = created.node_id,
                ));
            }
        }
    }

    // Create Party nodes and connect them to the Transaction
    for party in &requesting_parties {
        cypher_statements.push(cypher_query!(
            "MERGE (p:Party {party_id: $party_id})",
            party_id = party.clone(),
        ));
        cypher_statements.push(cypher_query!(
            "MATCH (p:Party {party_id: $party_id}), \
            (t:Transaction {offset: $offset}) \
            CREATE (p)-[:REQUESTED]->(t)",
            party_id = party.clone(),
            offset = offset,
        ));
    }

    cypher_statements
}
