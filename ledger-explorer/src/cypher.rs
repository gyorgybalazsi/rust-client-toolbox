use chrono::DateTime;
use client::utils::{
    extract_contract_ids_from_value, extract_edges, structure_markers_from_transaction,
};
use ledger_api::v2::{CreatedEvent, GetUpdatesResponse, get_updates_response::Update, event::Event};
use neo4rs::{Query, BoltType};
use serde_json::json;
use crate::api_record_to_json::{api_record_to_json, choice_argument_json};

/// Wrapper around neo4rs::Query that preserves the cypher string and params for debugging
#[derive(Clone)]
pub struct CypherQuery {
    pub cypher: String,
    pub params: Vec<(String, String)>,
    pub query: Query,
}

impl std::fmt::Debug for CypherQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CypherQuery")
            .field("cypher", &self.cypher)
            .field("params", &self.params)
            .finish()
    }
}

impl std::fmt::Display for CypherQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.cypher)?;
        if !self.params.is_empty() {
            write!(f, " [")?;
            for (i, (key, value)) in self.params.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}={}", key, value)?;
            }
            write!(f, "]")?;
        }
        Ok(())
    }
}

impl CypherQuery {
    pub fn new(cypher: String) -> Self {
        Self {
            query: Query::new(cypher.clone()),
            cypher,
            params: Vec::new(),
        }
    }

    pub fn with_param<T: Into<BoltType>>(mut self, key: &str, value: T) -> Self {
        self.query = self.query.param(key, value);
        self
    }

    pub fn with_json_param(mut self, key: &str, value: serde_json::Value) -> Self {
        // Convert serde_json::Value to BoltType
        let bolt_value: BoltType = value.try_into().unwrap_or(BoltType::Null(neo4rs::BoltNull));
        self.query = self.query.param(key, bolt_value);
        self
    }
}

macro_rules! cypher_query {
    ($cypher:expr, $($key:ident = $value:expr),* $(,)?) => {{
        let cypher_str = $cypher.to_string();
        let query = Query::new(cypher_str.clone())
            $(.param(stringify!($key), $value.clone()))*;
        let params = vec![
            $((stringify!($key).to_string(), format!("{:?}", $value))),*
        ];
        CypherQuery { cypher: cypher_str, params, query }
    }};
}

/// Converts a GetUpdatesResponse directly into a Vec of Cypher statements.
/// Uses UNWIND for batched operations to minimize round-trips.
/// Returns an empty vector if update is None or not a Transaction.
pub fn get_updates_response_to_cypher(response: &GetUpdatesResponse) -> Vec<CypherQuery> {
    let mut cypher_statements = Vec::new();

    let Some(update) = &response.update else {
        return cypher_statements;
    };

    let Update::Transaction(transaction) = update else {
        return cypher_statements;
    };

    let offset = transaction.offset;

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

    // Collect Created events for batch insert
    let mut created_events: Vec<serde_json::Value> = Vec::new();
    // Collect Exercised events for batch insert
    let mut exercised_events: Vec<serde_json::Value> = Vec::new();

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

                created_events.push(json!({
                    "contract_id": created.contract_id,
                    "template_name": template_name,
                    "label": label,
                    "signatories": signatories_str,
                    "offset": created.offset,
                    "node_id": created.node_id,
                    "created_at": created_at,
                    "create_arguments": create_arguments,
                    "create_arguments_json": create_arguments_json
                }));
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

                exercised_events.push(json!({
                    "label": label,
                    "choice_name": choice_name,
                    "target_contract_id": exercised.contract_id,
                    "acting_parties": acting_parties_str,
                    "offset": exercised.offset,
                    "node_id": exercised.node_id,
                    "consuming": exercised.consuming,
                    "result_contract_ids": serde_json::to_string(&extract_contract_ids_from_value(&exercised.exercise_result)).unwrap_or("[]".to_string()),
                    "last_descendant_node_id": exercised.last_descendant_node_id,
                    "transaction_effective_at": transaction_effective_at,
                    "choice_argument": choice_argument,
                    "choice_argument_json": choice_argument_json
                }));
            }
            _ => {}
        }
    }

    // Batch insert Created nodes using UNWIND
    if !created_events.is_empty() {
        let cypher = CypherQuery::new(
            "UNWIND $events AS e \
            CREATE (c:Created { \
            contract_id: e.contract_id, \
            template_name: e.template_name, \
            label: e.label, \
            signatories: e.signatories, \
            offset: e.offset, \
            node_id: e.node_id, \
            created_at: e.created_at, \
            create_arguments: e.create_arguments, \
            create_arguments_json: e.create_arguments_json })".to_string()
        ).with_json_param("events", serde_json::Value::Array(created_events));
        cypher_statements.push(cypher);
    }

    // Batch insert Exercised nodes using UNWIND
    if !exercised_events.is_empty() {
        let cypher = CypherQuery::new(
            "UNWIND $events AS e \
            CREATE (ex:Exercised { \
            label: e.label, \
            choice_name: e.choice_name, \
            target_contract_id: e.target_contract_id, \
            acting_parties: e.acting_parties, \
            offset: e.offset, \
            node_id: e.node_id, \
            consuming: e.consuming, \
            result_contract_ids: e.result_contract_ids, \
            last_descendant_node_id: e.last_descendant_node_id, \
            transaction_effective_at: e.transaction_effective_at, \
            choice_argument: e.choice_argument, \
            choice_argument_json: e.choice_argument_json })".to_string()
        ).with_json_param("events", serde_json::Value::Array(exercised_events));
        cypher_statements.push(cypher);
    }

    // Batch CONSEQUENCE edges
    let markers = structure_markers_from_transaction(transaction);
    let edges = extract_edges(&markers);
    if !edges.is_empty() {
        let edges_data: Vec<serde_json::Value> = edges
            .iter()
            .map(|(off, parent_id, child_id)| json!({
                "offset": off,
                "parent_id": parent_id,
                "child_id": child_id
            }))
            .collect();
        let cypher = CypherQuery::new(
            "UNWIND $edges AS e \
            MATCH (parent {offset: e.offset, node_id: e.parent_id}), \
            (child {offset: e.offset, node_id: e.child_id}) \
            CREATE (parent)-[:CONSEQUENCE]->(child)".to_string()
        ).with_json_param("edges", serde_json::Value::Array(edges_data));
        cypher_statements.push(cypher);
    }

    // Batch TARGET and CONSUMES relationships for Exercised events
    let mut target_rels: Vec<serde_json::Value> = Vec::new();
    let mut consumes_rels: Vec<serde_json::Value> = Vec::new();

    for event in &transaction.events {
        if let Some(Event::Exercised(exercised)) = &event.event {
            target_rels.push(json!({
                "offset": exercised.offset,
                "node_id": exercised.node_id,
                "target_contract_id": exercised.contract_id
            }));
            if exercised.consuming {
                consumes_rels.push(json!({
                    "offset": exercised.offset,
                    "node_id": exercised.node_id,
                    "target_contract_id": exercised.contract_id
                }));
            }
        }
    }

    if !target_rels.is_empty() {
        let cypher = CypherQuery::new(
            "UNWIND $rels AS r \
            MATCH (e:Exercised {offset: r.offset, node_id: r.node_id}), \
            (c:Created {contract_id: r.target_contract_id}) \
            CREATE (e)-[:TARGET]->(c)".to_string()
        ).with_json_param("rels", serde_json::Value::Array(target_rels));
        cypher_statements.push(cypher);
    }

    if !consumes_rels.is_empty() {
        let cypher = CypherQuery::new(
            "UNWIND $rels AS r \
            MATCH (e:Exercised {offset: r.offset, node_id: r.node_id}), \
            (c:Created {contract_id: r.target_contract_id}) \
            CREATE (e)-[:CONSUMES]->(c)".to_string()
        ).with_json_param("rels", serde_json::Value::Array(consumes_rels));
        cypher_statements.push(cypher);
    }

    // Identify root-level events (those not in any edge as a child)
    let child_node_ids: std::collections::HashSet<i32> = edges.iter().map(|(_, _, child)| *child).collect();

    // Collect root-level events for ACTION relationships
    let mut root_exercised: Vec<serde_json::Value> = Vec::new();
    let mut root_created: Vec<serde_json::Value> = Vec::new();
    let mut requesting_parties: std::collections::HashSet<String> = std::collections::HashSet::new();

    for event in &transaction.events {
        if let Some(Event::Exercised(exercised)) = &event.event {
            if !child_node_ids.contains(&exercised.node_id) {
                for party in &exercised.acting_parties {
                    requesting_parties.insert(party.clone());
                }
                root_exercised.push(json!({
                    "offset": offset,
                    "node_id": exercised.node_id
                }));
            }
        }
        if let Some(Event::Created(created)) = &event.event {
            if !child_node_ids.contains(&created.node_id) {
                for party in &created.signatories {
                    requesting_parties.insert(party.clone());
                }
                root_created.push(json!({
                    "offset": offset,
                    "node_id": created.node_id
                }));
            }
        }
    }

    // Batch ACTION relationships for root-level Exercised events
    if !root_exercised.is_empty() {
        let cypher = CypherQuery::new(
            "UNWIND $rels AS r \
            MATCH (t:Transaction {offset: r.offset}), \
            (e:Exercised {offset: r.offset, node_id: r.node_id}) \
            CREATE (t)-[:ACTION]->(e)".to_string()
        ).with_json_param("rels", serde_json::Value::Array(root_exercised));
        cypher_statements.push(cypher);
    }

    // Batch ACTION relationships for root-level Created events
    if !root_created.is_empty() {
        let cypher = CypherQuery::new(
            "UNWIND $rels AS r \
            MATCH (t:Transaction {offset: r.offset}), \
            (c:Created {offset: r.offset, node_id: r.node_id}) \
            CREATE (t)-[:ACTION]->(c)".to_string()
        ).with_json_param("rels", serde_json::Value::Array(root_created));
        cypher_statements.push(cypher);
    }

    // Batch Party MERGE and REQUESTED relationships
    if !requesting_parties.is_empty() {
        let parties: Vec<serde_json::Value> = requesting_parties
            .iter()
            .map(|p| json!({"party_id": p, "offset": offset}))
            .collect();

        // First MERGE all parties
        let merge_cypher = CypherQuery::new(
            "UNWIND $parties AS p \
            MERGE (party:Party {party_id: p.party_id})".to_string()
        ).with_json_param("parties", serde_json::Value::Array(parties.clone()));
        cypher_statements.push(merge_cypher);

        // Then create REQUESTED relationships
        let rel_cypher = CypherQuery::new(
            "UNWIND $parties AS p \
            MATCH (party:Party {party_id: p.party_id}), \
            (t:Transaction {offset: p.offset}) \
            CREATE (party)-[:REQUESTED]->(t)".to_string()
        ).with_json_param("parties", serde_json::Value::Array(parties));
        cypher_statements.push(rel_cypher);
    }

    cypher_statements
}

/// Converts a CreatedEvent (from ACS) into Cypher statements to create a Created node.
/// The offset is set to -1 to indicate this is from ACS (pre-existing contract).
/// The node_id is set to 0 since there's no transaction structure for ACS contracts.
pub fn created_event_to_cypher(created: &CreatedEvent) -> Vec<CypherQuery> {
    let mut cypher_statements = Vec::new();

    let label = created
        .template_id
        .as_ref()
        .map(|id| format!("{}@ACS", id.entity_name))
        .unwrap_or_else(|| "unknown@ACS".to_string());
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

    // Use MERGE to avoid duplicates if contract already exists
    cypher_statements.push(cypher_query!(
        "MERGE (c:Created { contract_id: $contract_id }) \
        ON CREATE SET \
        c.template_name = $template_name, \
        c.label = $label, \
        c.signatories = $signatories, \
        c.offset = $offset, \
        c.node_id = $node_id, \
        c.created_at = $created_at, \
        c.create_arguments = $create_arguments, \
        c.create_arguments_json = $create_arguments_json, \
        c.from_acs = true",
        contract_id = created.contract_id.clone(),
        template_name = template_name.clone(),
        label = label.clone(),
        signatories = signatories_str.clone(),
        offset = -1i64, // ACS contracts have no specific offset
        node_id = 0i32,
        created_at = created_at.clone(),
        create_arguments = create_arguments,
        create_arguments_json = create_arguments_json,
    ));

    cypher_statements
}
