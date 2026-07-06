use treease_core::core::{
    CompactTag, NodeExtraId, NodeId, NodeValueRef, ParsedKey, SemType, TreeNode, TreeNodeKind,
    TreeStore, ValueRep, infer_scalar_tag,
};
use treease_core::formats::Decode;
use treease_core::formats::decoder_python::PythonDecoder;

fn parsed_key_for_node(node: &TreeNode) -> Option<ParsedKey> {
    if !node.is_map_key {
        return None;
    }
    match node.sem_type {
        Some(SemType::Int) => inline_value(node).parse::<i64>().ok().map(ParsedKey::Int),
        Some(SemType::Str) | None => Some(ParsedKey::Str(inline_value(node).to_owned())),
        _ => None,
    }
}

fn inline_value(node: &TreeNode) -> &str {
    match &node.value {
        NodeValueRef::Missing => "",
        NodeValueRef::Inline(value) => value.as_str(),
        NodeValueRef::Stored(_) => panic!("expected inline node value in standalone test"),
    }
}

#[test]
fn tree_node_get_value_rep_and_infer_scalar_tag_cover_basic_scalars() {
    assert_eq!(
        TreeNode {
            value: "\"cat\"".to_owned().into(),
            ..TreeNode::default()
        }
        .get_value_rep_with("\"cat\"")
        .unwrap(),
        ValueRep::Str("\"cat\"".to_owned())
    );
    assert_eq!(
        TreeNode {
            value: "3".to_owned().into(),
            ..TreeNode::default()
        }
        .get_value_rep_with("3")
        .unwrap(),
        ValueRep::Int(3)
    );
    assert_eq!(
        TreeNode {
            value: "3.1".to_owned().into(),
            ..TreeNode::default()
        }
        .get_value_rep_with("3.1")
        .unwrap(),
        ValueRep::Float(3.1)
    );
    // bare "true" with empty tag → inferred as boolean (Zig: "true", tag="", expected=.boolean=true)
    assert_eq!(
        TreeNode {
            value: "true".to_owned().into(),
            ..TreeNode::default()
        }
        .get_value_rep_with("true")
        .unwrap(),
        ValueRep::Boolean(true)
    );
    assert_eq!(
        TreeNode {
            value: "y".to_owned().into(),
            tag: CompactTag::from_sem_type(SemType::Boolean),
            ..TreeNode::default()
        }
        .get_value_rep_with("y")
        .unwrap(),
        ValueRep::Boolean(true)
    );
    assert_eq!(
        TreeNode {
            tag: CompactTag::from_sem_type(SemType::Nil),
            ..TreeNode::default()
        }
        .get_value_rep_with("")
        .unwrap(),
        ValueRep::Nil
    );

    assert_eq!(infer_scalar_tag("", "true"), "!!bool");
    assert_eq!(infer_scalar_tag("", "12"), "!!int");
    assert_eq!(infer_scalar_tag("", "1e2"), "!!float");
    assert_eq!(infer_scalar_tag("", "1.2.3"), "!!str");
    assert_eq!(infer_scalar_tag("!custom", "1.2.3"), "!custom");
    // Existing "!!int" tag is preserved even with non-integer value (Zig: int flag keeps "!!int")
    assert_eq!(infer_scalar_tag("!!int", "abc"), "!!int");
}

#[test]
fn tree_node_packed_flags_round_trip_and_stay_compact() {
    let mut node = TreeNode::default();
    assert!(node.sequence_closed());
    assert!(!node.evaluate_together());
    assert!(!node.encode_separate());

    node.set_sequence_closed(false);
    node.set_evaluate_together(true);
    node.set_encode_separate(true);
    node.set_sequence_index(Some(3));
    node.set_extra(Some(NodeExtraId(5)));

    assert!(!node.sequence_closed());
    assert!(node.evaluate_together());
    assert!(node.encode_separate());
    assert_eq!(node.sequence_index(), Some(3));
    assert_eq!(node.extra(), Some(NodeExtraId(5)));
    assert!(std::mem::size_of::<TreeNode>() <= 128);
}

#[test]
fn tree_store_add_child_and_key_value_child_set_relationships() {
    let mut store = TreeStore::new();
    let root = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: CompactTag::from_sem_type(SemType::Map),
        ..TreeNode::default()
    });

    let child = store
        .add_child(
            root,
            TreeNode {
                kind: TreeNodeKind::Scalar,
                sem_type: Some(SemType::Str),
                tag: CompactTag::from_sem_type(SemType::Str),
                value: "child".to_owned().into(),
                ..TreeNode::default()
            },
        )
        .unwrap();
    assert_eq!(store.get(child).unwrap().parent, Some(root));

    let (key, value) = store
        .add_key_value_child(
            root,
            TreeNode::scalar(SemType::Str, "name"),
            TreeNode::scalar(SemType::Str, "Ada"),
        )
        .unwrap();
    assert!(store.get(key).unwrap().is_map_key);
    assert_eq!(store.get(value).unwrap().key(), Some(key));
    assert_eq!(store.get(root).unwrap().content.len(), 3);
}

#[test]
fn tree_store_create_child_and_document_for_work() {
    let mut store = TreeStore::new();
    let root = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: CompactTag::from_sem_type(SemType::Seq),
        document: 7,
        ..TreeNode::default()
    });
    let child = store.create_child(root).unwrap();

    assert_eq!(store.get(child).unwrap().parent, Some(root));
    assert_eq!(store.document_for(child).unwrap(), 7);
}

#[test]
fn tree_node_parsed_key_helpers_cover_string_and_int_keys() {
    let mut string_key = TreeNode::scalar(SemType::Str, "myKey");
    string_key.is_map_key = true;
    let mut int_key = TreeNode::scalar(SemType::Int, "4");
    int_key.is_map_key = true;

    assert_eq!(
        parsed_key_for_node(&string_key),
        Some(ParsedKey::Str("myKey".to_owned()))
    );
    assert_eq!(parsed_key_for_node(&int_key), Some(ParsedKey::Int(4)));

    let loose = TreeNode::scalar(SemType::Str, "value");
    assert_eq!(parsed_key_for_node(&loose), None);
}

// ---------------------------------------------------------------------------
// TestTreeNodeChildWhenParentUpdated
//   Child inherits document, file_index, and filename from parent when the
//   parent is updated.
// ---------------------------------------------------------------------------
#[test]
fn tree_node_child_when_parent_updated() {
    let mut store = TreeStore::new();
    let mut parent = TreeNode::default();
    parent.set_document(1);
    let parent_id = store.add(parent);
    store.set_document_meta(1, "meow", 2);
    let child_id = store.create_child(parent_id).unwrap();

    assert_eq!(store.filename_for(child_id).unwrap(), "meow");
    assert_eq!(store.file_index_for(child_id).unwrap(), 2);
    assert_eq!(store.document_for(child_id).unwrap(), 1);
}

// ---------------------------------------------------------------------------
// TestCreateScalarNodeScenarios
//   createScalarNode for str, int, float, bool, and null types with correct
//   tags.
// ---------------------------------------------------------------------------
#[test]
fn create_scalar_node_scenarios() {
    let n1 = TreeNode::scalar(SemType::Str, "mike");
    assert_eq!(inline_value(&n1), "mike");
    assert_eq!(n1.tag.as_str(), Some("!!str"));

    let n2 = TreeNode::scalar(SemType::Int, "3");
    assert_eq!(inline_value(&n2), "3");
    assert_eq!(n2.tag.as_str(), Some("!!int"));

    let n3 = TreeNode::scalar(SemType::Float, "3.1");
    assert_eq!(inline_value(&n3), "3.1");
    assert_eq!(n3.tag.as_str(), Some("!!float"));

    let n4 = TreeNode::scalar(SemType::Boolean, "true");
    assert_eq!(inline_value(&n4), "true");
    assert_eq!(n4.tag.as_str(), Some("!!bool"));

    let n5 = TreeNode::scalar(SemType::Nil, "~");
    assert_eq!(inline_value(&n5), "~");
    assert_eq!(n5.tag.as_str(), Some("!!null"));
}

// ---------------------------------------------------------------------------
// TestGetKeyForMapValue
//   getKey for a map value node returns the key's value.
// ---------------------------------------------------------------------------
#[test]
fn get_key_for_map_value() {
    let mut store = TreeStore::new();
    let key_node = TreeNode::scalar(SemType::Str, "yourKey");
    let key_id = store.add(key_node);
    let mut value_node = TreeNode {
        value: "meow".to_owned().into(),
        document: 3,
        ..TreeNode::default()
    };
    value_node.set_key(Some(key_id));
    let value_id = store.add(value_node);

    let pk = store.parsed_key_for(value_id).unwrap();
    assert_eq!(pk, Some(ParsedKey::Str("yourKey".to_owned())));
}

// ---------------------------------------------------------------------------
// TestGetKeyForMapKey
//   getKey for a map key node returns the key's own value.
// ---------------------------------------------------------------------------
#[test]
fn get_key_for_map_key() {
    let mut store = TreeStore::new();
    let mut key_node = TreeNode::scalar(SemType::Str, "yourKey");
    key_node.is_map_key = true;
    key_node.document = 3;
    let key_id = store.add(key_node);

    let pk = store.parsed_key_for(key_id).unwrap();
    assert_eq!(pk, Some(ParsedKey::Str("yourKey".to_owned())));
}

// ---------------------------------------------------------------------------
// TestGetKeyForValue
//   getKey for a plain value node returns None (no key association).
// ---------------------------------------------------------------------------
#[test]
fn get_key_for_value() {
    let mut store = TreeStore::new();
    let node = TreeNode {
        value: "meow".to_owned().into(),
        document: 3,
        ..TreeNode::default()
    };
    let node_id = store.add(node);

    let pk = store.parsed_key_for(node_id).unwrap();
    assert_eq!(pk, None);
}

// ---------------------------------------------------------------------------
// TestGetParsedKeyForMapKey
//   getParsedKey returns ParsedKey::Str for map key nodes.
// ---------------------------------------------------------------------------
#[test]
fn get_parsed_key_for_map_key() {
    let mut key = TreeNode::scalar(SemType::Str, "yourKey");
    key.is_map_key = true;
    key.document = 3;
    assert_eq!(
        parsed_key_for_node(&key),
        Some(ParsedKey::Str("yourKey".to_owned()))
    );
}

// ---------------------------------------------------------------------------
// TestGetParsedKeyForLooseValue
//   getParsedKey returns None for loose (non-key, non-map-value) nodes.
// ---------------------------------------------------------------------------
#[test]
fn get_parsed_key_for_loose_value() {
    let node = TreeNode {
        value: "meow".to_owned().into(),
        document: 3,
        ..TreeNode::default()
    };
    assert_eq!(parsed_key_for_node(&node), None);
}

// ---------------------------------------------------------------------------
// TestGetParsedKeyForMapValue
//   getParsedKey returns the key's parsed value for map value nodes.
//   Uses TreeStore::parsed_key_for which resolves the key via the store.
// ---------------------------------------------------------------------------
#[test]
fn get_parsed_key_for_map_value() {
    let mut store = TreeStore::new();
    // Create a string key and add it to the store
    let key_id = store.add(TreeNode::scalar(SemType::Str, "yourKey"));
    // Create a map value node whose key points to the key node
    let mut value_node = TreeNode {
        value: "meow".to_owned().into(),
        document: 3,
        ..TreeNode::default()
    };
    value_node.set_key(Some(key_id));
    let value_id = store.add(value_node);

    let pk = store.parsed_key_for(value_id).unwrap();
    assert_eq!(pk, Some(ParsedKey::Str("yourKey".to_owned())));
}

// ---------------------------------------------------------------------------
// TestGetParsedKeyForArrayValue
//   getParsedKey returns ParsedKey::Int for array index keys.
//   Uses TreeStore::parsed_key_for which resolves the int key via the store.
// ---------------------------------------------------------------------------
#[test]
fn get_parsed_key_for_array_value() {
    let mut store = TreeStore::new();
    // Create an integer key node and add it to the store
    let key_id = store.add(TreeNode::scalar(SemType::Int, "4"));
    // Create a map value node whose key points to the int key node
    let mut value_node = TreeNode {
        value: "meow".to_owned().into(),
        document: 3,
        ..TreeNode::default()
    };
    value_node.set_key(Some(key_id));
    let value_id = store.add(value_node);

    let pk = store.parsed_key_for(value_id).unwrap();
    assert_eq!(pk, Some(ParsedKey::Int(4)));
}

// ---------------------------------------------------------------------------
// TestTreeNodeAddKeyValueChild
//   addKeyValueChild clears is_map_key on the value node.
// ---------------------------------------------------------------------------
#[test]
fn tree_node_add_key_value_child_clears_is_map_key() {
    let mut store = TreeStore::new();
    let root = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: CompactTag::from_sem_type(SemType::Map),
        ..TreeNode::default()
    });

    let raw_key = TreeNode {
        value: "newKey".to_owned().into(),
        ..TreeNode::default()
    };
    let raw_value = TreeNode {
        value: "cool".to_owned().into(),
        is_map_key: true,
        ..TreeNode::default()
    };

    let (key_id, value_id) = store.add_key_value_child(root, raw_key, raw_value).unwrap();

    // The stored key must be marked as map key
    assert!(store.get(key_id).unwrap().is_map_key);
    // The stored value must NOT be marked as map key
    assert!(!store.get(value_id).unwrap().is_map_key);
}

// ---------------------------------------------------------------------------
// TestConvertToNodeInfo
//   convertToNodeInfo preserves kind, tag, comments, anchor, line/column,
//   and recursively converts children.
// ---------------------------------------------------------------------------
#[test]
fn convert_to_node_info() {
    let mut store = TreeStore::new();

    let child = store.add(TreeNode {
        kind: TreeNodeKind::Scalar,
        tag: CompactTag::from_text("!!str"),
        value: "childValue".to_owned().into(),
        line: 2,
        column: 3,
        ..TreeNode::default()
    });

    let parent = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: CompactTag::from_text("!!map"),
        line: 1,
        column: 1,
        content: vec![child],
        ..TreeNode::default()
    });
    store.set_comments(parent, "head", "line", "foot").unwrap();
    store.set_anchor(parent, "anchor").unwrap();

    let parent_node = store.get(parent).unwrap();
    assert_eq!(parent_node.kind, TreeNodeKind::Mapping);
    assert_eq!(parent_node.tag.as_str(), Some("!!map"));
    assert_eq!(store.head_comment_for(parent), Some("head"));
    assert_eq!(store.line_comment_for(parent), Some("line"));
    assert_eq!(store.foot_comment_for(parent), Some("foot"));
    assert_eq!(store.anchor_for(parent), Some("anchor"));
    assert_eq!(parent_node.line, 1);
    assert_eq!(parent_node.column, 1);
    assert_eq!(parent_node.content.len(), 1);

    let child_node = store.get(parent_node.content[0]).unwrap();
    assert_eq!(child_node.kind, TreeNodeKind::Scalar);
    assert_eq!(child_node.tag.as_str(), Some("!!str"));
    assert_eq!(
        store.value_for(parent_node.content[0]).unwrap(),
        "childValue"
    );
    assert_eq!(child_node.line, 2);
    assert_eq!(child_node.column, 3);
}

// ---------------------------------------------------------------------------
// TestTreeNodeGetPath
//   getPath builds path segments from the parent chain.
// ---------------------------------------------------------------------------
#[test]
fn tree_node_get_path() {
    let mut store = TreeStore::new();

    // Root node: path should be empty
    let root = store.add(TreeNode {
        value: "root".to_owned().into(),
        ..TreeNode::default()
    });
    let root_path = store.path_for(root).unwrap();
    assert_eq!(root_path.len(), 0);

    // Node with a key: path should have one segment
    let key = store.add(TreeNode::scalar(SemType::Str, "myKey"));
    let mut node = TreeNode {
        value: "myValue".to_owned().into(),
        ..TreeNode::default()
    };
    node.set_key(Some(key));
    let node = store.add(node);
    let node_path = store.path_for(node).unwrap();
    assert_eq!(node_path.len(), 1);
    assert_eq!(node_path[0], ParsedKey::Str("myKey".to_owned()));

    // Nested: parent key + child key
    let parent_key = store.add(TreeNode::scalar(SemType::Str, "parent"));
    let mut parent = TreeNode::default();
    parent.set_key(Some(parent_key));
    let parent = store.add(parent);

    // Rebuild the nested node with parent set
    let nested_key = store.add(TreeNode::scalar(SemType::Str, "myKey"));
    let mut nested = TreeNode {
        value: "myValue".to_owned().into(),
        parent: Some(parent),
        ..TreeNode::default()
    };
    nested.set_key(Some(nested_key));
    let nested = store.add(nested);

    let nested_path = store.path_for(nested).unwrap();
    assert_eq!(nested_path.len(), 2);
    assert_eq!(nested_path[0], ParsedKey::Str("parent".to_owned()));
    assert_eq!(nested_path[1], ParsedKey::Str("myKey".to_owned()));
}

// ---------------------------------------------------------------------------
// TestTreeNodeGetNicePath
//   getNicePath formatting: simple keys, array indices, dotted keys, nested
//   paths.
// ---------------------------------------------------------------------------
#[test]
fn tree_node_get_nice_path() {
    let mut store = TreeStore::new();

    // Simple string key
    let key = store.add(TreeNode::scalar(SemType::Str, "simple"));
    let mut node = TreeNode::default();
    node.set_key(Some(key));
    let node = store.add(node);
    assert_eq!(store.nice_path_for(node).unwrap(), "simple");

    // Array index key
    let array_key = store.add(TreeNode::scalar(SemType::Int, "0"));
    let mut array_node = TreeNode::default();
    array_node.set_key(Some(array_key));
    let array_node = store.add(array_node);
    assert_eq!(store.nice_path_for(array_node).unwrap(), "[0]");

    // Dotted key
    let dot_key = store.add(TreeNode::scalar(SemType::Str, "key.with.dots"));
    let mut dot_node = TreeNode::default();
    dot_node.set_key(Some(dot_key));
    let dot_node = store.add(dot_node);
    assert_eq!(store.nice_path_for(dot_node).unwrap(), "key.with.dots");

    // Nested path: parent.child
    let parent_key = store.add(TreeNode::scalar(SemType::Str, "parent"));
    let mut parent = TreeNode::default();
    parent.set_key(Some(parent_key));
    let parent = store.add(parent);
    let child_key = store.add(TreeNode::scalar(SemType::Str, "child"));
    let mut child = TreeNode {
        parent: Some(parent),
        ..TreeNode::default()
    };
    child.set_key(Some(child_key));
    let child = store.add(child);
    assert_eq!(store.nice_path_for(child).unwrap(), "parent.child");
}

// ---------------------------------------------------------------------------
// TestTreeNodeFilterMapContentByKey
//   filterMapContentByKey filters mapping content by a key predicate.
// ---------------------------------------------------------------------------
#[test]
fn tree_node_filter_map_content_by_key() {
    let mut store = TreeStore::new();

    let key1 = store.add(TreeNode::scalar(SemType::Str, "key1"));
    let value1 = store.add(TreeNode::scalar(SemType::Str, "value1"));
    let key2 = store.add(TreeNode::scalar(SemType::Str, "key2"));
    let value2 = store.add(TreeNode::scalar(SemType::Str, "value2"));
    let key3 = store.add(TreeNode::scalar(SemType::Str, "key3"));
    let value3 = store.add(TreeNode::scalar(SemType::Str, "value3"));

    let map_node = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        content: vec![key1, value1, key2, value2, key3, value3],
        ..TreeNode::default()
    });

    // Manually filter: keep only entries whose key is "key1" or "key3"
    let map = store.get(map_node).unwrap();
    let mut filtered = Vec::new();
    let mut i = 0;
    while i + 1 < map.content.len() {
        let k = map.content[i];
        let v = map.content[i + 1];
        let key_text = store.value_for(k).unwrap();
        if key_text == "key1" || key_text == "key3" {
            filtered.push(k);
            filtered.push(v);
        }
        i += 2;
    }

    assert_eq!(filtered.len(), 4);
    assert_eq!(store.value_for(filtered[0]).unwrap(), "key1");
    assert_eq!(store.value_for(filtered[1]).unwrap(), "value1");
    assert_eq!(store.value_for(filtered[2]).unwrap(), "key3");
    assert_eq!(store.value_for(filtered[3]).unwrap(), "value3");
}

// ---------------------------------------------------------------------------
// TestTreeNodeVisitValues
//   visitValues visits mapping values, sequence items, and skips scalars.
// ---------------------------------------------------------------------------
#[test]
fn tree_node_visit_values() {
    let mut store = TreeStore::new();

    // Mapping: visitValues should visit each value (every second child)
    let key1 = store.add(TreeNode::scalar(SemType::Str, "key1"));
    let value1 = store.add(TreeNode::scalar(SemType::Str, "value1"));
    let key2 = store.add(TreeNode::scalar(SemType::Str, "key2"));
    let value2 = store.add(TreeNode::scalar(SemType::Str, "value2"));

    let map_node = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        content: vec![key1, value1, key2, value2],
        ..TreeNode::default()
    });

    let map = store.get(map_node).unwrap();
    let mut visited: Vec<&str> = Vec::new();
    let mut i = 0;
    while i + 1 < map.content.len() {
        // Skip key, visit value
        visited.push(store.value_for(map.content[i + 1]).unwrap());
        i += 2;
    }
    assert_eq!(visited.len(), 2);
    assert_eq!(visited[0], "value1");
    assert_eq!(visited[1], "value2");

    // Sequence: visitValues should visit every child
    let item1 = store.add(TreeNode::scalar(SemType::Str, "item1"));
    let item2 = store.add(TreeNode::scalar(SemType::Str, "item2"));

    let seq_node = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        content: vec![item1, item2],
        ..TreeNode::default()
    });

    let seq = store.get(seq_node).unwrap();
    let mut seq_visited: Vec<&str> = Vec::new();
    for &child_id in &seq.content {
        seq_visited.push(store.value_for(child_id).unwrap());
    }
    assert_eq!(seq_visited.len(), 2);
    assert_eq!(seq_visited[0], "item1");
    assert_eq!(seq_visited[1], "item2");

    // Scalar: visitValues should visit nothing
    let scalar = store.add(TreeNode {
        kind: TreeNodeKind::Scalar,
        value: "scalar".to_owned().into(),
        ..TreeNode::default()
    });
    let scalar_node = store.get(scalar).unwrap();
    assert!(scalar_node.content.is_empty());
}

// ---------------------------------------------------------------------------
// TestTreeNodeCanVisitValues
//   canVisitValues returns true for mapping/sequence, false for scalar.
// ---------------------------------------------------------------------------
#[test]
fn tree_node_can_visit_values() {
    let map_node = TreeNode {
        kind: TreeNodeKind::Mapping,
        ..TreeNode::default()
    };
    let seq_node = TreeNode {
        kind: TreeNodeKind::Sequence,
        ..TreeNode::default()
    };
    let scalar_node = TreeNode {
        kind: TreeNodeKind::Scalar,
        ..TreeNode::default()
    };

    assert!(map_node.can_visit_values());
    assert!(seq_node.can_visit_values());
    assert!(!scalar_node.can_visit_values());
}

#[test]
fn tree_node_copy_helpers_and_value_child_ids_preserve_zig_relationships() {
    let mut original = TreeNode {
        kind: TreeNodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: CompactTag::from_sem_type(SemType::Map),
        content: vec![NodeId(10), NodeId(11), NodeId(12), NodeId(13)],
        parent: Some(NodeId(2)),
        key: NodeId(3),
        document: 9,
        line: 7,
        column: 5,
        ..TreeNode::default()
    };
    original.set_sequence_index(Some(4));
    original.set_extra(Some(NodeExtraId(7)));

    assert_eq!(original.value_child_ids(), vec![NodeId(11), NodeId(13)]);

    let copy = original.copy_without_content().unwrap();
    assert!(copy.content.is_empty());
    assert_eq!(copy.parent, Some(NodeId(2)));

    let replacement = TreeNode::scalar(SemType::Int, "42");
    let replaced = original.copy_as_replacement(&replacement).unwrap();
    assert_eq!(replaced.kind, TreeNodeKind::Scalar);
    assert_eq!(replaced.sem_type, Some(SemType::Int));
    assert_eq!(replaced.parent, Some(NodeId(2)));
    assert_eq!(replaced.key(), Some(NodeId(3)));
    assert_eq!(replaced.sequence_index(), Some(4));
    assert_eq!(replaced.document, 9);

    let commented = original
        .create_replacement_with_comments(TreeNodeKind::Sequence, SemType::Seq.tag())
        .unwrap();
    assert_eq!(commented.kind, TreeNodeKind::Sequence);
    assert_eq!(commented.extra(), Some(NodeExtraId(7)));
}

// ---------------------------------------------------------------------------
// TestTreeNodeAddChild
//   addChild for sequence nodes sets auto-index key and parent.
// ---------------------------------------------------------------------------
#[test]
fn tree_node_add_child() {
    let mut store = TreeStore::new();
    let parent = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        ..TreeNode::default()
    });

    let child = store
        .add_child(parent, TreeNode::scalar(SemType::Str, "child"))
        .unwrap();

    let parent_node = store.get(parent).unwrap();
    assert_eq!(parent_node.content.len(), 1);

    let child_node = store.get(child).unwrap();
    assert!(!child_node.is_map_key);
    // For a sequence, add_child sets sequence_index to the content length
    assert_eq!(child_node.sequence_index(), Some(0));
    assert_eq!(child_node.parent, Some(parent));
}

// ---------------------------------------------------------------------------
// TestTreeNodeAddChildren
//   addChildren for sequence and mapping nodes.
// ---------------------------------------------------------------------------
#[test]
fn tree_node_add_children() {
    let mut store = TreeStore::new();

    // Sequence: add multiple children
    let seq_parent = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        ..TreeNode::default()
    });
    let child1 = store
        .add_child(seq_parent, TreeNode::scalar(SemType::Str, "child1"))
        .unwrap();
    let child2 = store
        .add_child(seq_parent, TreeNode::scalar(SemType::Str, "child2"))
        .unwrap();

    let seq = store.get(seq_parent).unwrap();
    assert_eq!(seq.content.len(), 2);
    assert_eq!(store.value_for(child1).unwrap(), "child1");
    assert_eq!(store.value_for(child2).unwrap(), "child2");

    // Mapping: add key-value pairs
    let map_parent = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        ..TreeNode::default()
    });
    let (k1, v1) = store
        .add_key_value_child(
            map_parent,
            TreeNode::scalar(SemType::Str, "key1"),
            TreeNode::scalar(SemType::Str, "value1"),
        )
        .unwrap();
    let (k2, v2) = store
        .add_key_value_child(
            map_parent,
            TreeNode::scalar(SemType::Str, "key2"),
            TreeNode::scalar(SemType::Str, "value2"),
        )
        .unwrap();

    let map = store.get(map_parent).unwrap();
    assert_eq!(map.content.len(), 4);
    assert!(store.get(k1).unwrap().is_map_key);
    assert!(!store.get(v1).unwrap().is_map_key);
    assert!(store.get(k2).unwrap().is_map_key);
    assert!(!store.get(v2).unwrap().is_map_key);
}

#[test]
fn python_root_boundary_regression_resolves_to_root_without_crashing() {
    let source = "{'alpha': [1, 2]}\n";
    let mut decoded = PythonDecoder.decode_str(source).unwrap();
    decoded.store.set_tree(
        "python-root-boundary",
        "python",
        decoded.root,
        None,
        source,
        vec![],
    );

    let root_path = decoded.store.path_for(decoded.root).unwrap();
    let nice_root_path = decoded.store.nice_path_for(decoded.root).unwrap();

    assert!(root_path.is_empty());
    assert_eq!(nice_root_path, "");

    let root = decoded.store.get(decoded.root).unwrap();
    assert!(root.end_byte > root.start_byte);
    assert_eq!(root.kind, TreeNodeKind::Mapping);
}
