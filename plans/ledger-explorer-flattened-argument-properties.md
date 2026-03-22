# Plan: Ledger Explorer — Flatten Arguments to Neo4j Properties

## Context

Currently, `Created` and `Exercised` Neo4j nodes store create/choice arguments in two ways:
- `create_arguments` / `choice_argument` — raw protobuf JSON blob
- `create_arguments_json` / `choice_argument_json` — human-readable nested JSON

Both are opaque strings. To query individual field values (e.g., "find all contracts where `person.homeAddress.city = 'Zurich'`"), users must parse the JSON in Cypher or post-process results. This is slow and awkward.

### Goal

Flatten nested DAML Record values into **dot-separated Neo4j node properties** so they become directly queryable:

```
// Before (Created node)
create_arguments_json: '{"admin":"Alice","person":{"name":"Bob","homeAddress":{"city":"Zurich"}}}'

// After (Created node) — all of the above PLUS:
create_arg.admin: "Alice"
create_arg.person.name: "Bob"
create_arg.person.homeAddress.city: "Zurich"
create_arg.person.homeAddress.street: "123 Main St"
create_arg.person.homeAddress.zip: "8001"
create_arg.person.age: 30
```

### Branch Dependency

**This can be built on `main`.** The ledger explorer operates on runtime gRPC `Value`/`Record` protobuf types, not on codegen-generated Rust structs. The flattening logic reads the `RecordField.label` and `Value.sum` from the wire — no compile-time type information is needed. The `feat/extend-codegen` branch changes the codegen crate only, which the ledger explorer does not depend on.

---

## 1. Add `flatten_record_to_properties`

**File**: `ledger-explorer/src/api_record_to_json.rs`

New function that recursively walks a `Record` and produces dot-separated key-value pairs:

```rust
/// Flattens a DAML Record into dot-separated property pairs.
///
/// Example: Record { person: Record { name: "Alice", age: 30 } }
/// → [("person.name", json!("Alice")), ("person.age", json!(30))]
pub fn flatten_record_to_properties(
    record: &Record,
    prefix: &str,
    max_depth: usize,
) -> Vec<(String, serde_json::Value)>
```

**Return type**: `Vec<(String, serde_json::Value)>` — not `BoltType`. The existing code uses `serde_json::Value` throughout the param pipeline (`with_json_param` converts to `BoltType` internally via `neo4rs`'s `TryFrom<serde_json::Value> for BoltType`). Returning `serde_json::Value` keeps the function consistent with the rest of the codebase and avoids an unnecessary `BoltType` dependency in this module.

### Flattening rules per Value variant

| Value variant | Behavior | Example key | Example value |
|---|---|---|---|
| `Text(s)` | Emit string | `create_arg.name` | `json!("Alice")` |
| `Int64(i)` | Emit integer | `create_arg.age` | `json!(30)` |
| `Bool(b)` | Emit boolean | `create_arg.active` | `json!(true)` |
| `Numeric(n)` | Emit string (Neo4j has no decimal type) | `create_arg.price` | `json!("100.50")` |
| `Party(p)` | Emit string | `create_arg.admin` | `json!("Alice::1220...")` |
| `ContractId(c)` | Emit string | `create_arg.ref` | `json!("00...")` |
| `Date(d)` | Emit integer (days since epoch) | `create_arg.date` | `json!(19437)` |
| `Timestamp(t)` | Emit integer (micros since epoch) | `create_arg.ts` | `json!(...)` |
| `Record(r)` | **Recurse** with extended prefix | `create_arg.person.name` | (recursive) |
| `Optional(Some(v))` | Unwrap and recurse | `create_arg.desc` | (inner value) |
| `Optional(None)` | Emit null | `create_arg.desc` | `json!(null)` |
| `List(l)` | Emit as JSON string (Neo4j lists can't hold mixed types) | `create_arg.items` | `json!("[...]")` |
| `TextMap(m)` | Emit as JSON string | `create_arg.metadata` | `json!("{...}")` |
| `GenMap(m)` | Emit as JSON string | `create_arg.mapping` | `json!("{...}")` |
| `Variant(v)` | Emit constructor string at key; if payload is non-Unit, also flatten payload at `.value` sub-prefix | `create_arg.shape` = `json!("Circle")`, `create_arg.shape.value` = `json!("100.50")` (primitive) or `create_arg.shape.value.radius` (Record field) |
| `Enum(e)` | Emit constructor as string | `create_arg.color` | `json!("Red")` |
| `Unit` | Skip (no meaningful value) | — | — |

### Prefix convention

- Created events: prefix = `"create_arg."` — explicit about which arguments these are
- Exercised events: prefix = `"choice_arg."` — distinguishes from create args

The prefix also prevents collisions with existing node properties (e.g., a Record field named `offset` becomes `create_arg.offset`, not `offset`).

### Depth limit

Accept `max_depth: usize` parameter (default: 10). At max depth, emit the remaining value as a JSON string via `api_value_to_json()`. This prevents runaway recursion on deeply nested structures.

---

## 2. Add `flatten_value_to_properties`

**File**: `ledger-explorer/src/api_record_to_json.rs`

For exercised events, `choice_argument` is a `Value` (not always a `Record`). Need a parallel function:

```rust
pub fn flatten_value_to_properties(
    value: &Value,
    prefix: &str,
    max_depth: usize,
) -> Vec<(String, serde_json::Value)>
```

If the value is a `Record`, delegate to `flatten_record_to_properties`. Otherwise, emit a single property at the prefix key using the same variant rules from step 1.

---

## 3. Update Cypher Generation for Created Events (UNWIND path)

**File**: `ledger-explorer/src/cypher.rs` (lines 179-261)

### Current flow
```
created.create_arguments → serde_json::to_string() → single "create_arguments" property
created.create_arguments → api_record_to_json() → serde_json::to_string() → single "create_arguments_json" property
```

### New flow (additive)
```
created.create_arguments → flatten_record_to_properties("create_arg.", max_depth)
    → collect into serde_json::Value::Object → passed as "flattened_args" in the UNWIND array
```

### Dynamic properties via `SET n += map`

Neo4j's `UNWIND ... SET` syntax requires property names to be known at query-compile time. But our properties are dynamic (different templates have different fields).

**Solution**: Use `SET c += p.flattened_args`. Neo4j 4.x+ supports `+=` with map values. The `neo4rs` crate converts `serde_json::Value::Object` → `BoltMap` via its `TryFrom<serde_json::Value> for BoltType` implementation (verified in `neo4rs-0.9.0-rc.8/src/convert.rs`).

**Verification step**: Before implementing, test this Cypher pattern in a standalone Neo4j query to confirm `+=` works inside `UNWIND`:
```cypher
UNWIND [{id: "test", props: {foo: "bar", baz: 42}}] AS p
MERGE (n:Test {id: p.id})
SET n += p.props
RETURN n
```

### Implementation

In the Created event processing (cypher.rs ~line 191):

```rust
let flattened_args: serde_json::Value = created
    .create_arguments
    .as_ref()
    .map(|args| {
        let props = flatten_record_to_properties(args, "create_arg.", max_depth);
        serde_json::Value::Object(props.into_iter().collect())
    })
    .unwrap_or(json!({}));

created_events.push(json!({
    "contract_id": created.contract_id,
    // ... existing fields unchanged ...
    "create_arguments": create_arguments,
    "create_arguments_json": create_arguments_json,
    "flattened_args": flattened_args
}));
```

Update the Cypher string to append `c += p.flattened_args` after the existing `ON CREATE SET` properties.

---

## 4. Update Cypher Generation for Exercised Events (UNWIND path)

**File**: `ledger-explorer/src/cypher.rs` (lines 220-282)

Same pattern as Created, but using `flatten_value_to_properties` with prefix `"choice_arg."`:

```rust
let flattened_args: serde_json::Value = exercised
    .choice_argument
    .as_ref()
    .map(|arg| {
        let props = flatten_value_to_properties(arg, "choice_arg.", max_depth);
        serde_json::Value::Object(props.into_iter().collect())
    })
    .unwrap_or(json!({}));

exercised_events.push(json!({
    // ... existing fields unchanged ...
    "choice_argument": choice_argument,
    "choice_argument_json": choice_argument_json,
    "flattened_args": flattened_args
}));
```

Update the Exercised Cypher string to append `e += p.flattened_args`.

---

## 5. Update ACS Loading (non-UNWIND path)

**File**: `ledger-explorer/src/cypher.rs` (function `created_event_to_cypher`, ~line 464)

The ACS loader uses the `cypher_query!` macro with individual `$key = $value` params — **not** the `UNWIND` + JSON array pattern. This requires a different integration:

1. Compute `flattened_args` the same way as step 3
2. Pass it as a separate param: `.with_json_param("flattened_args", flattened_args)`
3. Append `c += $flattened_args` to the Cypher string

```rust
let flattened_args: serde_json::Value = created
    .create_arguments
    .as_ref()
    .map(|args| {
        let props = flatten_record_to_properties(args, "create_arg.", max_depth);
        serde_json::Value::Object(props.into_iter().collect())
    })
    .unwrap_or(json!({}));

let mut query = cypher_query!(
    "MERGE (c:Created { contract_id: $contract_id }) \
    ON CREATE SET \
    c.template_name = $template_name, \
    ... \
    c.from_acs = true, \
    c += $flattened_args",
    contract_id = created.contract_id.clone(),
    // ... existing params ...
);
// cypher_query! macro doesn't support json params, so use with_json_param:
query = query.with_json_param("flattened_args", flattened_args);
```

**Note**: The `cypher_query!` macro may not support mixing with `with_json_param`. If not, convert this function to use the same `CypherQuery::new()` + `with_json_param()` builder pattern used by the UNWIND path.

---

## 6. Migration / Backfill for Existing Nodes

Both the stream and ACS Cypher use `MERGE ... ON CREATE SET`. If a `Created` or `Exercised` node already exists from a previous sync run (before flattening was enabled), the new flattened properties **will not be added** — `ON CREATE SET` only fires on new nodes.

### Options

1. **Change to `SET` (no `ON CREATE` guard)**: Always set all properties, including flattened. This is safe because MERGE still prevents duplicate nodes, and the values are deterministic (same arguments → same flattened properties). **This is the simplest approach.**

2. **Add `ON MATCH SET c += p.flattened_args`**: Only update flattened properties on existing nodes, don't touch other properties. More surgical but adds Cypher complexity.

3. **One-time backfill script**: Read `create_arguments_json` from existing nodes, flatten client-side, write back. Only needed if options 1-2 are rejected.

**Recommendation**: Option 1 — use `SET` instead of `ON CREATE SET` for the `+= flattened_args` line. Keep `ON CREATE SET` for the static properties to avoid unnecessary writes.

```cypher
UNWIND $props AS p
MERGE (c:Created { contract_id: p.contract_id })
ON CREATE SET
  c.template_name = p.template_name,
  c.label = p.label,
  ...
SET c += p.flattened_args
```

The `SET` (without `ON CREATE`) runs on both create and match, ensuring existing nodes get backfilled on next encounter.

---

## 7. Configuration

**File**: `ledger-explorer/src/config.rs`

Add to `ConfigFile` (top-level, since this is a storage behavior, not ledger-specific):

```rust
#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    pub logging: LoggingConfig,
    pub neo4j: Neo4jConfig,
    pub active_profile: String,
    pub profiles: HashMap<String, ProfileConfig>,
    #[serde(default = "default_storage")]
    pub storage: StorageConfig,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    /// Whether to flatten create/choice arguments into dot-separated Neo4j properties
    #[serde(default = "default_true")]
    pub flatten_arguments: bool,
    /// Maximum recursion depth for flattening nested records
    #[serde(default = "default_max_depth")]
    pub flatten_max_depth: usize,
}
```

TOML:
```toml
[storage]
flatten_arguments = true
flatten_max_depth = 10
```

---

## 8. Keep Existing JSON Properties

**Important**: The existing `create_arguments`, `create_arguments_json`, `choice_argument`, `choice_argument_json` properties are **preserved**. The flattened properties are **additive**. Reasons:

- The JSON blob is useful for full reconstruction
- The human-readable JSON is useful for display
- Flattened properties enable direct Cypher queries
- Removing them would break existing queries/dashboards

---

## 9. Neo4j Indexes for Flattened Properties

**File**: `ledger-explorer/src/sync.rs` (index creation block, ~lines 56-66)

Don't pre-create indexes for flattened properties — their names are dynamic and template-specific. Instead:

- Document how users can add indexes for their specific templates:
  ```cypher
  CREATE INDEX IF NOT EXISTS FOR (c:Created) ON (c.`create_arg.admin`)
  ```
- Optionally (follow-up): add a config option `auto_index_flattened_properties: bool` that creates indexes for all observed `create_arg.*` properties after the first batch.

---

## Files to Modify

| File | Changes |
|------|---------|
| `ledger-explorer/src/api_record_to_json.rs` | Add `flatten_record_to_properties`, `flatten_value_to_properties` |
| `ledger-explorer/src/cypher.rs` | Update Created/Exercised event processing to include `flattened_args` map; update Cypher `SET` to use `+= p.flattened_args`; update `created_event_to_cypher` for ACS with `with_json_param` |
| `ledger-explorer/src/config.rs` | Add `StorageConfig` with `flatten_arguments` and `flatten_max_depth` |
| `ledger-explorer/config/config.toml.example` | Add `[storage]` section |

## Implementation Order

1. Verify `SET n += map` works inside UNWIND in a standalone Neo4j query
2. Add `flatten_record_to_properties` and `flatten_value_to_properties` with unit tests
3. Add `StorageConfig` to config
4. Update UNWIND path for Created events (cypher.rs batch processing)
5. Update UNWIND path for Exercised events
6. Update ACS path (`created_event_to_cypher`)
7. `cargo check -p ledger-explorer`
8. Integration test against sandbox + Neo4j

## Verification

1. `cargo check -p ledger-explorer` — compiles
2. Unit test: `flatten_record_to_properties` on a nested Record produces expected dot-separated keys
3. Unit test: variant flattening — constructor at key, payload at `.value` sub-key
4. Unit test: optional None → null, optional Some → unwrapped value
5. Unit test: depth limit produces JSON string at max depth
6. Integration: run ledger-explorer against sandbox with nested-test DAR, verify Neo4j node has `create_arg.person.homeAddress.city` property
7. Verify existing `create_arguments_json` properties are unchanged
8. Cypher query: `MATCH (c:Created) WHERE c.\`create_arg.person.homeAddress.city\` = 'Zurich' RETURN c` returns results
9. Verify backfill: re-run sync on existing data, confirm flattened properties appear on previously-created nodes

## Edge Cases

- **Empty Record**: No flattened properties emitted (just the JSON blobs)
- **List/Map values**: Stored as JSON strings since Neo4j properties can't hold arbitrary nested structures
- **Variant with primitive payload**: Constructor name at `create_arg.shape`, payload value at `create_arg.shape.value`
- **Variant with Record payload**: Constructor name at `create_arg.shape`, Record fields at `create_arg.shape.value.radius`, `create_arg.shape.value.height`, etc.
- **Variant with Unit payload**: Constructor name only, no `.value` sub-key
- **Duplicate field paths**: Not possible — DAML Records have unique field labels within each level
- **Very long field paths**: Truncated at max depth, remainder stored as JSON string
- **Special characters in field names**: Neo4j property names support any UTF-8 string when backtick-quoted; no sanitization needed
- **`flatten_arguments = false`**: Skip flattening entirely, emit empty `{}` for `flattened_args` (existing JSON properties still written)
