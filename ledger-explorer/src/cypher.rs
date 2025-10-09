use chrono::DateTime;
use client::utils::{
    extract_contract_ids_from_value, extract_edges, structure_markers_from_transaction,
};
use ledger_api::v2::{GetUpdatesResponse, get_updates_response::Update, event::Event};
use neo4rs::{Query, query};
use crate::api_record_to_json::{api_record_to_json, choice_argument_json};

/// Converts a GetUpdatesResponse directly into a Vec of Cypher statements.
/// Returns an empty vector if update is None or not a Transaction.
pub fn get_updates_response_to_cypher(response: &GetUpdatesResponse) -> Vec<Query> {
    let mut cypher_statements = Vec::new();

    let Some(update) = &response.update else {
        return cypher_statements;
    };

    let Update::Transaction(transaction) = update else {
        return cypher_statements;
    };

    for event in &transaction.events {
        match &event.event {
            Some(Event::Created(created)) => {
                let label = created
                    .template_id
                    .as_ref()
                    .map(|id| format!("{}@{}", created.offset, id.entity_name))
                    .unwrap_or_else(|| format!("unknown@{:?}", created.offset));
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
                cypher_statements.push(query!(
                    "CREATE (c:Created \
                    {{ contract_id: {contract_id}, \
                    template_name: {template_name}, \
                    label: {label}, \
                    signatories: {signatories}, \
                    offset: {offset}, \
                    node_id: {node_id}, \
                    created_at: {created_at}, \
                    create_arguments: {create_arguments}, \
                    create_arguments_json: {create_arguments_json} }})",
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
                let label = format!("{}@{}", exercised.offset, exercised.choice);
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
                cypher_statements.push(query!(
                    "CREATE (e:Exercised \
                    {{ label: {label}, \
                    choice_name: {choice_name}, \
                    target_contract_id: {target_contract_id}, \
                    acting_parties: {acting_parties}, \
                    offset: {offset}, \
                    node_id: {node_id}, \
                    consuming: {consuming}, \
                    result_contract_ids: {result_contract_ids}, \
                    last_descendant_node_id: {last_descendant_node_id}, \
                    transaction_effective_at: {transaction_effective_at}, \
                    choice_argument: {choice_argument}, \
                    choice_argument_json: {choice_argument_json} }})",
                    label = label.clone(),
                    choice_name = choice_name.clone(),
                    target_contract_id = exercised.contract_id.clone(),
                    acting_parties = acting_parties_str.clone(),
                    offset = exercised.offset,
                    node_id = exercised.node_id,
                    consuming = exercised.consuming,
                    result_contract_ids = format!("{:?}", extract_contract_ids_from_value(&exercised.exercise_result)),
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
    for (offset, parent_id, child_id) in edges {
        // TODO why the query! macro does not work here?
        // cypher_statements.push(query!(
        //     "MATCH (parent {{offset: {offset}, node_id: {parent_id}}}), (child {{offset: {offset}, node_id: {child_id}}}) CREATE (parent)-[:CONSEQUENCE]->(child);",
        //     offset = offset,
        //     parent_id = parent_id as i64,
        //     child_id = child_id as i64,
        // ));
        let query = format!(
            "MATCH (parent \
            {{offset: {offset}, \
            node_id: {parent_id}}}), \
            (child {{offset: {offset}, \
            node_id: {child_id}}}) \
            CREATE (parent)-[:CONSEQUENCE]->(child);",
            offset = offset,
            parent_id = parent_id,
            child_id = child_id,
        );
        cypher_statements.push(Query::new(query));
    }

    // Add TARGET relationships: from Exercised node to Created node with matching contract_id
    for event in &transaction.events {
        if let Some(Event::Exercised(exercised)) = &event.event {
            cypher_statements.push(query!(
                "MATCH (e:Exercised \
                {{offset: {offset}, \
                node_id: {node_id}}}), \
                (c:Created {{contract_id: {target_contract_id}}}) \
                CREATE (e)-[:TARGET]->(c);",
                offset = exercised.offset,
                node_id = exercised.node_id,
                target_contract_id = exercised.contract_id.clone(),
            ));
            if exercised.consuming {
                cypher_statements.push(query!(
                    "MATCH (e:Exercised \
                    {{offset: {offset}, \
                    node_id: {node_id}}}), \
                    (c:Created {{contract_id: {target_contract_id}}}) \
                    CREATE (e)-[:CONSUMES]->(c);",
                    offset = exercised.offset,
                    node_id = exercised.node_id,
                    target_contract_id = exercised.contract_id.clone(),
                ));
            }
        }
    }

    cypher_statements
}
