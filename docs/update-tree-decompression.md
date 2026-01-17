# Update Tree Decompression Algorithm

This document describes the algorithm used to reconstruct parent-child relationships from the flat event stream returned by the Canton Ledger API.

## Background

Canton 3.3 introduced a new encoding for transaction trees called "Universal Event Streams". Instead of storing explicit parent-child relationships (`root_event_ids` and `child_event_ids`), the new format uses a more compact interval-based encoding with two integer fields per node:

- **`node_id`**: An integer identifying the node's position within transaction execution
- **`last_descendant_node_id`**: The upper boundary for the node IDs of events that are consequences of this exercised event

This encoding is a variant of the **Nested Set Model** (also known as Modified Preorder Tree Traversal or MPTT), which represents hierarchical data using interval containment rather than explicit pointers.

## How the Encoding Works

### Nested Set Model Principle

In the nested set model, each node is assigned a range `[node_id, last_descendant_node_id]`. A node A is a **descendant** of node B if and only if:

```
A.node_id > B.node_id AND A.node_id <= B.last_descendant_node_id
```

Equivalently, A's interval is contained within B's interval.

### Example Transaction Tree

Consider a transaction where Alice accepts a TicketOffer, which triggers a Cash transfer:

```
Accept (node_id=0, last_descendant=5)
├── Transfer (node_id=2, last_descendant=4)
│   ├── Created Cash (node_id=3, last_descendant=3)
│   └── Archive Cash (node_id=4, last_descendant=4)
└── Created TicketAgreement (node_id=5, last_descendant=5)
```

Note: `node_id=1` may be filtered out (not visible to this party).

The intervals are:
| Node | Interval | Children's Intervals |
|------|----------|---------------------|
| Accept | [0, 5] | Contains [2,4] and [5,5] |
| Transfer | [2, 4] | Contains [3,3] and [4,4] |
| Created Cash | [3, 3] | Leaf (no children) |
| Archive Cash | [4, 4] | Leaf (no children) |
| Created TicketAgreement | [5, 5] | Leaf (no children) |

## Algorithm Implementation

The algorithm in `client/src/utils.rs` reconstructs parent-child edges using a stack-based approach:

```rust
pub fn extract_edges(markers: &[StructureMarker]) -> Vec<(i64, i32, i32)> {
    // Sort markers by node_id to ensure traversal order
    let mut sorted = markers.to_vec();
    sorted.sort_by_key(|m| m.node_id);

    let mut stack: Vec<(i64, i32, i32)> = Vec::new(); // (offset, node_id, last_descendant)
    let mut edges: Vec<(i64, i32, i32)> = Vec::new();

    for marker in &sorted {
        // Pop nodes whose descendants are already processed
        while let Some(&(_, _, last_desc)) = stack.last() {
            if last_desc < marker.node_id {
                stack.pop();
            } else {
                break;
            }
        }

        // If there's a parent on the stack, add an edge
        if let Some(&(_, parent_id, _)) = stack.last() {
            edges.push((marker.offset, parent_id, marker.node_id));
        }

        // Push the current node onto the stack
        stack.push((marker.offset, marker.node_id, marker.last_descendant_node_id));
    }

    edges
}
```

### Algorithm Walkthrough

Using the example above:

1. **Process node_id=0**: Stack empty → no parent. Push (0, 5). Stack: `[(0,5)]`
2. **Process node_id=2**: `last_desc=5 >= 2` → don't pop. Parent=0. Edge `(0→2)`. Push (2, 4). Stack: `[(0,5), (2,4)]`
3. **Process node_id=3**: `last_desc=4 >= 3` → don't pop. Parent=2. Edge `(2→3)`. Push (3, 3). Stack: `[(0,5), (2,4), (3,3)]`
4. **Process node_id=4**: `last_desc=3 < 4` → pop (3,3). `last_desc=4 >= 4` → don't pop. Parent=2. Edge `(2→4)`. Push (4, 4). Stack: `[(0,5), (2,4), (4,4)]`
5. **Process node_id=5**: `last_desc=4 < 5` → pop (4,4). `last_desc=4 < 5` → pop (2,4). `last_desc=5 >= 5` → don't pop. Parent=0. Edge `(0→5)`. Push (5, 5). Stack: `[(0,5), (5,5)]`

**Resulting edges**: `(0→2)`, `(2→3)`, `(2→4)`, `(0→5)` ✓

## Key Properties

### Leaf Nodes

`CreatedEvent` nodes are always leaf nodes and do not have a `last_descendant_node_id` field in the protobuf. The implementation sets `last_descendant_node_id = node_id` for these events, correctly indicating they have no children.

### Filtered Nodes

The algorithm handles gaps in `node_id` sequences (when intermediate nodes are filtered out due to party visibility). The interval containment property is preserved even when some nodes are missing.

### Root Node Identification

A node is a **root node** if it has no ancestors in the visible event set. Root nodes are identified as those whose `node_id` does not fall within any other node's interval. In the implementation, these are nodes that remain at the bottom of the stack after processing, or nodes processed when the stack is empty.

### Complexity

- **Time**: O(n log n) for sorting, O(n) for the stack-based traversal
- **Space**: O(n) for the sorted copy and stack

## Comparison with Digital Asset's Java Implementation

The official Digital Asset Java bindings use a recursive approach to build the tree:

```java
private static void buildNodeTree(Node parent, LinkedList<Node> nodes) {
  while (!nodes.isEmpty() &&
         nodes.peekFirst().nodeId <= parent.lastDescendantNodeId) {
    parent.children.add(nodes.peekFirst());
    buildNodeTree(nodes.pollFirst(), nodes);
  }
}
```

Both implementations use the **same underlying algorithm**: the nested set model where a node is a descendant of another if its `node_id` falls within the parent's `[node_id, last_descendant_node_id]` interval. The parent-child relationships produced are identical.

### Why the Rust implementation uses a different style

| Aspect | Java (recursive) | Rust (iterative with stack) |
|--------|------------------|----------------------------|
| Stack usage | Implicit call stack | Explicit heap-allocated stack |
| Stack overflow risk | Yes, on deep trees | No |
| Output format | Mutable `Node` objects with `children` lists | Immutable edge list `Vec<(offset, parent_id, child_id)>` |
| Flexibility | Tree structure only | Edge list can build any representation |

The iterative approach with an explicit stack is idiomatic Rust and avoids potential stack overflow on deeply nested transaction trees. The edge list output is more flexible—consumers can build tree structures, use it for graph algorithms, or process edges directly without additional transformation.

## Related Representations

This encoding is related to:

- **DFUDS (Depth-First Unary Degree Sequence)**: A succinct tree representation that encodes trees in execution order
- **Nested Set Model**: Uses `(left, right)` values from a depth-first traversal; the Canton variant uses `(node_id, last_descendant_node_id)`
- **Interval Trees**: Data structures for storing intervals and querying containment

## References

1. **Digital Asset Java Bindings** - Official tree decompression implementation
   https://github.com/digital-asset/daml/blob/main/sdk/canton/community/bindings-java/src/main/java/com/daml/ledger/javaapi/data/Transaction.java

2. **Canton/Daml 3.3 Release Notes** - Universal Event Streams and tree encoding changes
   https://blog.digitalasset.com/developers/release-notes/splice-0.4.0-canton-3.3#a_universal_event-Streams

3. **Nested Set Model** - ScienceDirect Computer Science Topics
   https://www.sciencedirect.com/topics/computer-science/nested-set-model

4. **Nested Set Model** - Wikipedia
   https://en.wikipedia.org/wiki/Nested_set_model

5. **Canton Ledger API Protobuf** - `ExercisedEvent.last_descendant_node_id` field definition
   `ledger-api/resources/protobuf/com/daml/ledger/api/v2/event.proto`
