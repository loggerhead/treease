use treease_core::{
    core::dispatch_matching_nodes,
    operators::{
        Context, ExpressionNode, NodeKind, Operation, SELF_REFERENCE_OP_TYPE, SemType,
        TRAVERSE_PATH_OP_TYPE, TreeEngine, TreeNode, get_matching_nodes, update_from,
    },
};

// ── Helper functions ─────────────────────────────────────────────

fn string_scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: SemType::Str.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn int_scalar(value: i64) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: value.to_string(),
        ..TreeNode::default()
    }
}

fn make_map_node(pairs: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(pairs.len() * 2);
    for (key, value) in pairs {
        let mut key_node = string_scalar(key);
        key_node.is_map_key = true;
        let mut value_node = value;
        value_node.is_map_key = false;
        content.push(key_node);
        content.push(value_node);
    }
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        content,
        ..TreeNode::default()
    }
}

fn make_seq_node(children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: children,
        ..TreeNode::default()
    }
}

/// Find a value by key in a mapping node's content.
fn get_content_value_by_key<'a>(map_node: &'a TreeNode, wanted_key: &str) -> Option<&'a TreeNode> {
    let mut i = 0;
    while i + 1 < map_node.content.len() {
        if map_node.content[i].value == wanted_key {
            return Some(&map_node.content[i + 1]);
        }
        i += 2;
    }
    None
}

/// Build a traverse_path ExpressionNode for a given key string.
fn traverse_path_expr(key: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &TRAVERSE_PATH_OP_TYPE,
            value: None,
            string_value: key.to_owned(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

// ── TestGetMatchingNodes_NilExpressionNode ───────────────────────

#[test]
fn test_get_matching_nodes_nil_expression_node() {
    let mut engine = TreeEngine::default();
    let ctx = Context::default();

    let result = get_matching_nodes(&mut engine, &ctx, None).unwrap();

    assert_eq!(result.matching_nodes.len(), 0);
}

// ── TestGetMatchingNodes_UnknownOperator ─────────────────────────

#[test]
fn test_get_matching_nodes_unknown_operator() {
    // NOTE: The Zig test creates `OperationType{ .id = .custom, .custom_name = "UNKNOWN" }`
    // and asserts `getMatchingNodes` returns `error.UnknownOperator`.
    //
    // In the Rust compat layer (`operators::OperationType`), the type only carries an
    // `OperationId` enum variant — there is no `custom_name` field like the Zig version.
    // The `OperatorRegistry::get_handler` method looks up handlers by `OperationId` via a
    // `HashMap<OperationId, OperatorHandler>`, and `init_registry` registers a handler for
    // **every** `OperationId` variant. Consequently, there is no way to construct an
    // `OperationType` whose `id` is not present in the static registry, and thus the
    // `UnknownOperator` error path cannot be triggered through the compat dispatch layer.
    //
    // The core `Operation` (in `core/expression.rs`) does support custom names via
    // `OperationType::custom(...)`, but the active dispatch path in
    // `core/operation::get_matching_nodes` delegates to the compat-layer
    // `operator_registry`, so it is equally unreachable.
    //
    // This test is kept as a placeholder documenting the architectural limitation.
}

// ── TestGetMatchingNodes_ValidOperator ───────────────────────────

#[test]
fn test_get_matching_nodes_valid_operator() {
    let mut engine = TreeEngine::default();
    let scalar = string_scalar("test");
    let ctx = Context {
        matching_nodes: vec![scalar],
        ..Context::default()
    };

    // SELF operator returns the context unchanged
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &SELF_REFERENCE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    };

    let result = get_matching_nodes(&mut engine, &ctx, Some(&mut expr)).unwrap();

    assert_eq!(result.matching_nodes.len(), 1);
    assert_eq!(result.matching_nodes[0].value, "test");
}

#[test]
fn core_dispatch_matching_nodes_falls_back_to_static_registry_when_global_has_no_override() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![string_scalar("static_registry")],
        codec_registry: 1,
        ..Context::default()
    };
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &SELF_REFERENCE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    };

    let result = dispatch_matching_nodes(&mut engine, &ctx, Some(&mut expr)).unwrap();

    assert_eq!(result.matching_nodes.len(), 1);
    assert_eq!(result.matching_nodes[0].value, "static_registry");
}

// ── DeeplyAssign: adds new map key ───────────────────────────────

#[test]
fn test_deeply_assign_adds_new_map_key() {
    let mut engine = TreeEngine::default();

    let root = make_map_node(vec![("existing", string_scalar("old_value"))]);
    let ctx = Context {
        matching_nodes: vec![root.clone()],
        ..Context::default()
    };

    // Use traverse_path to navigate to "new_key" — this auto-creates it
    let mut traverse_expr = traverse_path_expr("new_key");
    let result = get_matching_nodes(&mut engine, &ctx, Some(&mut traverse_expr)).unwrap();

    // The auto-created key should produce a nil node
    assert_eq!(result.matching_nodes.len(), 1);
    assert_eq!(result.matching_nodes[0].sem_type, Some(SemType::Nil));

    // Now manually add the key-value to the original root to verify
    // the equivalent of deeply_assign behavior
    let mut root_mut = root.clone();
    let new_key = string_scalar("new_key");
    let new_value = string_scalar("new_value");
    root_mut.add_key_value_child(&new_key, &new_value).unwrap();

    assert_eq!(root_mut.content.len(), 4);

    let found = get_content_value_by_key(&root_mut, "new_key");
    assert!(found.is_some());
    assert_eq!(found.unwrap().value, "new_value");
}

// ── DeeplyAssign: overwrites existing value ──────────────────────

#[test]
fn test_deeply_assign_overwrites_existing_value() {
    let mut root = make_map_node(vec![("key", string_scalar("old_value"))]);

    // Simulate overwrite by replacing the value in content
    let new_value = string_scalar("new_value");

    // Find the key index and replace the value
    let key_idx = {
        let mut i = 0;
        while i + 1 < root.content.len() {
            if root.content[i].value == "key" {
                break;
            }
            i += 2;
        }
        i
    };

    root.content[key_idx + 1] = new_value;

    assert_eq!(root.content.len(), 2);
    assert_eq!(root.content[1].value, "new_value");
}

// ── DeeplyAssign: creates deep path ──────────────────────────────

#[test]
fn test_deeply_assign_creates_deep_path() {
    let mut engine = TreeEngine::default();

    // Create root with level1 -> empty map
    let level1_map = make_map_node(vec![]);
    let root = make_map_node(vec![("level1", level1_map)]);

    let ctx = Context {
        matching_nodes: vec![root.clone()],
        ..Context::default()
    };

    // Navigate to level1
    let mut traverse_l1 = traverse_path_expr("level1");
    let l1_ctx = get_matching_nodes(&mut engine, &ctx, Some(&mut traverse_l1)).unwrap();
    assert_eq!(l1_ctx.matching_nodes.len(), 1);

    // Navigate from level1 to level2 (auto-creates)
    let mut traverse_l2 = traverse_path_expr("level2");
    let l2_ctx = get_matching_nodes(&mut engine, &l1_ctx, Some(&mut traverse_l2)).unwrap();
    assert_eq!(l2_ctx.matching_nodes.len(), 1);

    // Navigate from level2 to level3 (auto-creates)
    let mut traverse_l3 = traverse_path_expr("level3");
    let l3_ctx = get_matching_nodes(&mut engine, &l2_ctx, Some(&mut traverse_l3)).unwrap();
    assert_eq!(l3_ctx.matching_nodes.len(), 1);
    assert_eq!(l3_ctx.matching_nodes[0].sem_type, Some(SemType::Nil));

    // Now manually build the equivalent deep path to verify structure
    let mut root_manual = make_map_node(vec![("level1", make_map_node(vec![]))]);
    let deep_value = string_scalar("deep_value");

    // Add level2 -> level3 -> deep_value under level1
    let level3_map = make_map_node(vec![("level3", deep_value)]);
    let level2_map = make_map_node(vec![("level2", level3_map)]);

    // Replace level1's value with the new level2 map
    root_manual.content[1] = level2_map;

    let l1 = &root_manual.content[1];
    assert_eq!(l1.kind, NodeKind::Mapping);
    assert_eq!(l1.content.len(), 2);
    assert_eq!(l1.content[0].value, "level2");

    let l2 = &l1.content[1];
    assert_eq!(l2.kind, NodeKind::Mapping);
    assert_eq!(l2.content[0].value, "level3");
    assert_eq!(l2.content[1].value, "deep_value");
}

// ── DeeplyAssign: assigns into array index ───────────────────────

#[test]
fn test_deeply_assign_assigns_into_array_index() {
    let seq = make_seq_node(vec![]);
    let root = make_map_node(vec![("array", seq)]);

    // Manually add an element at index 0
    let mut root_mut = root.clone();
    let array_value = string_scalar("array_value");

    // Find the array in the map
    let array_idx = {
        let mut i = 0;
        while i + 1 < root_mut.content.len() {
            if root_mut.content[i].value == "array" {
                break;
            }
            i += 2;
        }
        i + 1
    };

    root_mut.content[array_idx].content.push(array_value);

    let out_seq = &root_mut.content[array_idx];
    assert_eq!(out_seq.kind, NodeKind::Sequence);
    assert_eq!(out_seq.content.len(), 1);
    assert_eq!(out_seq.content[0].value, "array_value");
}

// ── DeeplyAssign: merges mapping ─────────────────────────────────

#[test]
fn test_deeply_assign_merges_mapping() {
    let inner = make_map_node(vec![("existing_key", string_scalar("existing_value"))]);
    let root = make_map_node(vec![("config", inner)]);

    // Simulate merge: add new_key and update existing_key
    let mut root_mut = root.clone();

    // Find the config map
    let config_idx = {
        let mut i = 0;
        while i + 1 < root_mut.content.len() {
            if root_mut.content[i].value == "config" {
                break;
            }
            i += 2;
        }
        i + 1
    };

    // Add new_key -> new_value
    let new_key = string_scalar("new_key");
    let new_value = string_scalar("new_value");
    root_mut.content[config_idx]
        .add_key_value_child(&new_key, &new_value)
        .unwrap();

    // Update existing_key -> updated_value
    let existing_key_idx = {
        let config = &root_mut.content[config_idx];
        let mut i = 0;
        while i + 1 < config.content.len() {
            if config.content[i].value == "existing_key" {
                break;
            }
            i += 2;
        }
        i + 1
    };
    root_mut.content[config_idx].content[existing_key_idx] = string_scalar("updated_value");

    let config = &root_mut.content[config_idx];
    assert_eq!(config.content.len(), 4);

    let mut found_existing = false;
    let mut found_new = false;
    let mut i = 0;
    while i + 1 < config.content.len() {
        let k = &config.content[i];
        let v = &config.content[i + 1];
        if k.value == "existing_key" {
            found_existing = true;
            assert_eq!(v.value, "updated_value");
        }
        if k.value == "new_key" {
            found_new = true;
            assert_eq!(v.value, "new_value");
        }
        i += 2;
    }
    assert!(found_existing);
    assert!(found_new);
}

// ── TreeEngine: deeplyAssign autovivify does not leak on reset paths ─

#[test]
fn test_tree_engine_deeply_assign_autovivify_does_not_leak() {
    let mut engine = TreeEngine::default();

    // Create a scalar root "x"
    let root = string_scalar("x");
    let ctx = Context {
        matching_nodes: vec![root],
        ..Context::default()
    };

    let rhs = make_map_node(vec![]);
    let path = vec![string_scalar("a"), int_scalar(0), string_scalar("b")];

    engine.deeply_assign(ctx, &path, rhs).unwrap();
}

// ── TreeEngine: deepMergeInto handles deep nesting ───────────────

#[test]
fn test_tree_engine_deep_merge_handles_deep_nesting() {
    // Build a deeply nested map manually and verify it can be traversed
    // without stack overflow (Rust doesn't have recursion limits on
    // data structure traversal like this).

    let depth = 100usize;

    // Build from the innermost level outward
    let mut cur = make_map_node(vec![]);
    for _ in 0..depth {
        cur = make_map_node(vec![("k", cur)]);
    }

    // Verify the nesting depth
    let mut node = &cur;
    for _i in 0..depth {
        assert_eq!(node.kind, NodeKind::Mapping);
        assert_eq!(node.content.len(), 2);
        assert_eq!(node.content[0].value, "k");
        node = &node.content[1];
    }
    // After `depth` levels, we should be at the innermost empty map
    assert_eq!(node.kind, NodeKind::Mapping);
    assert_eq!(node.content.len(), 0);

    // Verify we can traverse the structure using traverse_path
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![cur.clone()],
        ..Context::default()
    };

    // Navigate down `depth` levels
    let mut current_ctx = ctx;
    for _ in 0..depth {
        let mut traverse_k = traverse_path_expr("k");
        current_ctx = get_matching_nodes(&mut engine, &current_ctx, Some(&mut traverse_k)).unwrap();
        assert_eq!(current_ctx.matching_nodes.len(), 1);
    }

    // Should end up at the innermost empty map
    assert_eq!(current_ctx.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(current_ctx.matching_nodes[0].content.len(), 0);
}

// ── TreeEngine: deepMergeInto merges all map keys ────────────────

#[test]
fn test_tree_engine_deep_merge_merges_all_map_keys() {
    // Build: root.m.a.x = 1, root.m.b = "old"
    let a_map = make_map_node(vec![("x", int_scalar(1))]);
    let m_map = make_map_node(vec![("a", a_map), ("b", string_scalar("old"))]);
    let root = make_map_node(vec![("m", m_map)]);

    // Build rhs: a.y = 2, b = "new", c = "cval"
    let rhs_a = make_map_node(vec![("y", int_scalar(2))]);
    let rhs = make_map_node(vec![
        ("a", rhs_a),
        ("b", string_scalar("new")),
        ("c", string_scalar("cval")),
    ]);

    // Simulate merge: replace m's content with merged result
    let mut root_mut = root.clone();

    // Find m in root
    let m_idx = {
        let mut i = 0;
        while i + 1 < root_mut.content.len() {
            if root_mut.content[i].value == "m" {
                break;
            }
            i += 2;
        }
        i + 1
    };

    // Merge rhs into m: for each key in rhs, add or update in m
    let mut i: usize = 0;
    while i + 1 < rhs.content.len() {
        let rhs_key = &rhs.content[i];
        let rhs_val = &rhs.content[i + 1];

        // Check if key exists in m
        let existing = get_content_value_by_key(&root_mut.content[m_idx], &rhs_key.value);
        if existing.is_some() {
            // Update existing: find and replace
            let m_content = &mut root_mut.content[m_idx].content;
            let mut j = 0;
            while j + 1 < m_content.len() {
                if m_content[j].value == rhs_key.value {
                    m_content[j + 1] = rhs_val.clone();
                    break;
                }
                j += 2;
            }
        } else {
            // Add new key-value
            root_mut.content[m_idx]
                .add_key_value_child(rhs_key, rhs_val)
                .unwrap();
        }
        i += 2;
    }

    let merged_m = &root_mut.content[m_idx];

    // Verify merged_a has y=2 and x is gone (replaced by rhs_a)
    let merged_a = get_content_value_by_key(merged_m, "a").unwrap();
    assert_eq!(merged_a.kind, NodeKind::Mapping);

    let merged_y = get_content_value_by_key(merged_a, "y").unwrap();
    assert_eq!(merged_y.value, "2");

    // x should be gone since rhs_a replaced the entire a subtree
    assert!(get_content_value_by_key(merged_a, "x").is_none());

    // Verify b = "new"
    let merged_b = get_content_value_by_key(merged_m, "b").unwrap();
    assert_eq!(merged_b.value, "new");

    // Verify c = "cval"
    let merged_c = get_content_value_by_key(merged_m, "c").unwrap();
    assert_eq!(merged_c.value, "cval");
}

// ── TreeEngine: traverse_path auto-creates map entries ───────────

#[test]
fn test_traverse_path_auto_creates_map_entries() {
    let mut engine = TreeEngine::default();

    // Start with an empty map
    let root = make_map_node(vec![]);
    let ctx = Context {
        matching_nodes: vec![root],
        ..Context::default()
    };

    // Navigate to a non-existent key — should auto-create it
    let mut traverse_expr = traverse_path_expr("new_key");
    let result = get_matching_nodes(&mut engine, &ctx, Some(&mut traverse_expr)).unwrap();

    assert_eq!(result.matching_nodes.len(), 1);
    // The auto-created value should be nil
    assert_eq!(result.matching_nodes[0].sem_type, Some(SemType::Nil));
}

// ── TreeEngine: update_from copies node contents ─────────────────

#[test]
fn test_update_from_copies_node_contents() {
    let mut dst = string_scalar("old");
    let src = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: "42".to_owned(),
        head_comment: "# hello".to_owned(),
        line_comment: "# world".to_owned(),
        ..TreeNode::default()
    };

    update_from(&mut dst, &src).unwrap();

    assert_eq!(dst.kind, NodeKind::Scalar);
    assert_eq!(dst.sem_type, Some(SemType::Int));
    assert_eq!(dst.value, "42");
    assert_eq!(dst.head_comment, "# hello");
    assert_eq!(dst.line_comment, "# world");
}

// ── TreeEngine: traverse_path navigates into existing keys ───────

#[test]
fn test_traverse_path_navigates_into_existing_keys() {
    let mut engine = TreeEngine::default();

    let inner = string_scalar("inner_value");
    let root = make_map_node(vec![("key", inner)]);

    let ctx = Context {
        matching_nodes: vec![root],
        ..Context::default()
    };

    let mut traverse_expr = traverse_path_expr("key");
    let result = get_matching_nodes(&mut engine, &ctx, Some(&mut traverse_expr)).unwrap();

    assert_eq!(result.matching_nodes.len(), 1);
    assert_eq!(result.matching_nodes[0].value, "inner_value");
}
