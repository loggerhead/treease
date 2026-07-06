use treease_core::core::authoritative_graph_service::{
    clear_document_state, get_authoritative_tree,
    get_document_analysis as facade_get_document_analysis, get_document_state,
    store_authoritative_graph, store_owned_document_analysis,
};
use treease_core::core::graph_builder::{
    GraphEdge, GraphKind, GraphModel as BuilderGraphModel, GraphNode, GraphNodeKey,
};
use treease_core::core::{
    CompactTag, DocumentAnalysisDemand, GraphFragmentIndex, LineIndex, NodeId, SemType,
    StoredDocumentAnalysisOwned, TreeNode, TreeNodeKind, TreeStore, analyze_document_internal,
    analyze_document_internal_with_demand, store_transient_document_analysis,
};

#[test]
fn tree_store_add_and_get_nodes_preserve_kind_and_value() {
    let mut store = TreeStore::new();

    let scalar_id = store.add(TreeNode::scalar(SemType::Str, "value"));
    let map_id = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        ..TreeNode::default()
    });

    assert_eq!(scalar_id, NodeId(0));
    assert_eq!(map_id, NodeId(1));
    assert_eq!(store.len(), 2);
    assert_eq!(store.get(scalar_id).unwrap().kind, TreeNodeKind::Scalar);
    assert_eq!(store.value_for(scalar_id).unwrap(), "value");
    assert_eq!(store.get(map_id).unwrap().kind, TreeNodeKind::Mapping);
}

#[test]
fn tree_store_get_mut_updates_existing_node_in_place() {
    let mut store = TreeStore::new();
    let id = store.add(TreeNode::scalar(SemType::Int, "1"));

    store.get_mut(id).unwrap().tag = CompactTag::from_text("!!custom");
    store.set_value(id, "42").unwrap();

    assert_eq!(store.get(id).unwrap().tag.as_str(), Some("!!custom"));
    assert_eq!(store.value_for(id).unwrap(), "42");
}

#[test]
fn tree_store_indices_remain_stable_as_nodes_are_appended() {
    let mut store = TreeStore::new();
    let first = store.add(TreeNode::scalar(SemType::Int, "1"));
    let second = store.add(TreeNode::scalar(SemType::Int, "2"));

    assert_eq!(first, NodeId(0));
    assert_eq!(second, NodeId(1));
    assert_eq!(store.value_for(first).unwrap(), "1");
    assert_eq!(store.value_for(second).unwrap(), "2");
}

#[test]
fn tree_store_rebuilds_value_index_after_discard_without_changing_dedup_semantics() {
    let mut store = TreeStore::new();
    let first = store.add(TreeNode::scalar(SemType::Str, "shared"));

    store.discard_value_index();

    let second = store.add(TreeNode::scalar(SemType::Str, "shared"));
    let third = store.add(TreeNode::scalar(SemType::Str, "other"));
    store.set_value(third, "shared").unwrap();

    let first_value = store.value_ref_for(first).unwrap();
    let second_value = store.value_ref_for(second).unwrap();
    let third_value = store.value_ref_for(third).unwrap();

    assert_eq!(first_value, second_value);
    assert_eq!(first_value, third_value);
    assert_eq!(store.value_for(second).unwrap(), "shared");
}

#[test]
fn tree_store_add_child_sets_parent_and_sequence_index_for_sequences() {
    let mut store = TreeStore::new();
    let parent = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        ..TreeNode::default()
    });

    let first = store
        .add_child(parent, TreeNode::scalar(SemType::Int, "1"))
        .unwrap();
    let second = store
        .add_child(parent, TreeNode::scalar(SemType::Int, "2"))
        .unwrap();

    assert_eq!(store.get(parent).unwrap().content, vec![first, second]);
    assert_eq!(store.get(first).unwrap().parent, Some(parent));
    assert_eq!(store.get(first).unwrap().sequence_index(), Some(0));
    assert_eq!(store.get(second).unwrap().sequence_index(), Some(1));
}

#[test]
fn tree_store_add_key_value_child_marks_key_and_links_value_back_to_key() {
    let mut store = TreeStore::new();
    let parent = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        ..TreeNode::default()
    });

    let (key, value) = store
        .add_key_value_child(
            parent,
            TreeNode::scalar(SemType::Str, "name"),
            TreeNode::scalar(SemType::Str, "Ada"),
        )
        .unwrap();

    assert_eq!(store.get(parent).unwrap().content, vec![key, value]);
    assert!(store.get(key).unwrap().is_map_key);
    assert_eq!(store.get(key).unwrap().parent, Some(parent));
    assert_eq!(store.get(value).unwrap().key(), Some(key));
    assert_eq!(store.get(value).unwrap().parent, Some(parent));
}

#[test]
fn tree_store_set_get_remove_clear_for_trees_and_views() {
    let mut store = TreeStore::new();

    // setTree / getTree
    let root1 = store.add(TreeNode::scalar(SemType::Str, "one"));
    store.set_tree("a", "json", root1, None, "source-one", vec![]);
    assert_eq!(store.get_tree("a"), Some(root1));

    // Overwrite with same key, different language
    let root2 = store.add(TreeNode::scalar(SemType::Str, "two"));
    store.set_tree("a", "python", root2, None, "source-two", vec![]);
    assert_eq!(store.get_tree("a"), Some(root2));

    // removeTree
    assert!(!store.remove_tree("missing"));
    assert!(store.remove_tree("a"));
    assert_eq!(store.get_tree("a"), None);

    // setGraph / getGraph
    let model1 = BuilderGraphModel::default();
    store.set_graph("v", model1);
    assert!(store.get_graph("v").is_some());

    // setGraphWithIndex / getGraphIndex
    let model2 = BuilderGraphModel::default();
    let index2 = GraphFragmentIndex::default();
    store.set_graph_with_index("v", model2, Some(index2));
    assert_eq!(store.get_graph("v").unwrap().nodes.len(), 0);
    assert!(store.get_graph_index("v").is_some());

    // removeGraph
    assert!(!store.remove_graph("missing"));
    assert!(store.remove_graph("v"));
    assert!(store.get_graph("v").is_none());

    // clear
    let root3 = store.add(TreeNode::scalar(SemType::Str, "three"));
    store.set_tree("b", "yaml", root3, None, "source-three", vec![]);
    let model3 = BuilderGraphModel::default();
    store.set_graph("w", model3);

    store.clear();
    assert_eq!(store.get_tree("b"), None);
    assert!(store.get_graph("w").is_none());
}

#[test]
fn authoritative_graph_service_exposes_unified_document_state_over_tree_store() {
    let mut store = TreeStore::new();

    // setDocumentAnalysis
    let root = store.add(TreeNode::scalar(SemType::Str, "value"));
    let diagnostics_raw = vec![1, 2, 3, 4, 5];
    let semantic_tokens = vec![9];
    let value_json = "{".to_string();
    store.set_document_analysis(
        "doc",
        "json",
        root,
        None,
        "{\"v\":1}",
        vec![],
        diagnostics_raw,
        semantic_tokens,
        value_json,
    );

    // storeAuthoritativeGraph equivalent: set_graph_with_index
    let model = BuilderGraphModel::default();
    let index = GraphFragmentIndex::default();
    store.set_graph_with_index("doc", model, Some(index));

    // getDocumentState equivalent: verify tree entry and graph entry
    let entry = store.get_tree_entry("doc").unwrap();
    assert_eq!(entry.root, root);
    assert_eq!(entry.source, "{\"v\":1}");
    assert!(store.get_graph("doc").is_some());
    assert!(store.get_graph_index("doc").is_some());

    // clearDocumentState equivalent: remove_tree + remove_graph
    store.remove_tree("doc");
    store.remove_graph("doc");
    assert!(store.get_tree_entry("doc").is_none());
    assert!(store.get_graph("doc").is_none());
}

#[test]
fn store_transient_document_analysis_preserves_streaming_token_spans() {
    let mut analysis = analyze_document_internal("json", br#"{"a":"b"}"#, false);
    let stored = analysis
        .stored
        .as_ref()
        .expect("streaming json analysis should produce stored artifacts");

    assert!(!stored.token_spans.is_empty());
    assert!(!stored.semantic_tokens_encoded.is_empty());

    let mut store = TreeStore::new();
    store_transient_document_analysis(&mut store, "stream-doc", &mut analysis);

    let stored = store
        .get_document_analysis("stream-doc")
        .expect("stored analysis should be available");
    assert!(!stored.token_spans.is_empty());
    assert!(!stored.semantic_tokens_encoded.is_empty());
    assert!(analysis.stored.is_none());
}
#[test]
fn diagnostics_only_analysis_matches_parse_failed_diagnostics_without_stored_artifacts() {
    let full = analyze_document_internal("json", br#"{"a": }"#, false);
    let diagnostics_only = analyze_document_internal_with_demand(
        "json",
        br#"{"a": }"#,
        false,
        DocumentAnalysisDemand::diagnostics_only(),
    );

    assert_eq!(diagnostics_only.diagnostics_raw, full.diagnostics_raw);
    assert!(diagnostics_only.stored.is_none());
}

#[test]
fn diagnostics_only_analysis_skips_stored_artifacts_for_valid_nonstreaming_source() {
    let diagnostics_only = analyze_document_internal_with_demand(
        "yaml",
        b"a: 1\n",
        false,
        DocumentAnalysisDemand::diagnostics_only(),
    );

    assert!(diagnostics_only.diagnostics_raw.is_empty());
    assert!(diagnostics_only.stored.is_none());
}
#[test]
fn diagnostics_only_analysis_handles_bom_prefixed_json_eof_errors() {
    let diagnostics_only = analyze_document_internal_with_demand(
        "json",
        "\u{feff}{\"a\":".as_bytes(),
        false,
        DocumentAnalysisDemand::diagnostics_only(),
    );

    assert!(!diagnostics_only.diagnostics_raw.is_empty());
    assert!(diagnostics_only.stored.is_none());
}
#[test]
fn diagnostics_only_analysis_reports_rootless_json_as_error() {
    let diagnostics_only = analyze_document_internal_with_demand(
        "json",
        "\u{feff}   ".as_bytes(),
        false,
        DocumentAnalysisDemand::diagnostics_only(),
    );

    assert!(!diagnostics_only.diagnostics_raw.is_empty());
    assert!(diagnostics_only.stored.is_none());
    assert!(diagnostics_only.stored.is_none());
}

#[test]
fn authoritative_graph_service_facade_reads_and_clears_document_state() {
    let mut store = TreeStore::new();
    let root = store.add(TreeNode::scalar(SemType::Str, "value"));
    store.set_document_analysis(
        "doc",
        "json",
        root,
        None,
        "{\"v\":1}",
        vec![],
        vec![1, 2, 3, 4, 5],
        vec![9, 8, 7],
        "{\"v\":1}".to_string(),
    );

    let model = BuilderGraphModel::default();
    let index = GraphFragmentIndex::default();
    store_authoritative_graph(&mut store, "doc", model.clone(), Some(index.clone()));

    let state = get_document_state(&store, "doc").expect("document state should exist");
    assert_eq!(state.analysis.root, root);
    assert_eq!(state.analysis.source, "{\"v\":1}");
    assert!(state.graph_model.is_some());
    assert!(state.fragment_index.is_some());
    assert_eq!(get_authoritative_tree(&store, "doc"), Some(root));

    let analysis =
        facade_get_document_analysis(&store, "doc").expect("facade document analysis should exist");
    assert_eq!(analysis.root, root);
    assert_eq!(analysis.source, "{\"v\":1}");
    assert_eq!(analysis.semantic_tokens_encoded, &[9, 8, 7]);

    clear_document_state(&mut store, "doc");
    assert!(get_document_state(&store, "doc").is_none());
    assert!(store.get_tree_entry("doc").is_none());
    assert!(store.get_graph("doc").is_none());
}

#[test]
fn authoritative_graph_service_facade_stores_owned_document_analysis() {
    let mut store = TreeStore::new();
    let root = store.add(TreeNode::scalar(SemType::Str, "value"));
    let source = "{\"v\":1}";

    store_owned_document_analysis(
        &mut store,
        "doc",
        StoredDocumentAnalysisOwned {
            language: "json".to_string(),
            root,
            source: source.as_bytes().to_vec(),
            ts_tree: None,
            token_spans: vec![],
            diagnostics_raw: vec![1, 2, 3, 4, 5],
            semantic_tokens_encoded: vec![9, 8, 7],
            value_json: Some(source.to_string()),
            line_index: LineIndex::build(source),
        },
    );

    let state = get_document_state(&store, "doc").expect("document state should exist");
    assert_eq!(state.analysis.root, root);
    assert_eq!(state.analysis.source, source);
    assert_eq!(state.analysis.diagnostics_raw, &[1, 2, 3, 4, 5]);
    assert_eq!(state.analysis.semantic_tokens_encoded, &[9, 8, 7]);
    assert_eq!(state.analysis.value_json, source);
}

#[test]
fn graph_fragment_index_falls_back_to_missing_parent_render_handle_for_parent_stable_id() {
    let empty_key = GraphNodeKey {
        stable_id: 0,
        path: vec![],
    };
    let model = BuilderGraphModel::from_parts(
        vec![GraphNode {
            render_handle: 7,
            stable_id: 0,
            key: empty_key.clone(),
            kind: GraphKind::Scalar,
            depth: 1,
            x: 0,
            y: 0,
            width: 10,
            height: 10,
            box_args: Default::default(),
            path: vec![],
            meta: Default::default(),
            rows: vec![],
            table: None,
            preorder_first: 7,
            preorder_last: 7,
            source: None,
        }],
        vec![GraphEdge {
            from_render_handle: 42,
            from_key: empty_key.clone(),
            from_row: -1,
            to_render_handle: 7,
            to_key: empty_key,
            to_row: -1,
            bezier_args: Default::default(),
        }],
    );

    let index = GraphFragmentIndex::build(&model);
    let fragment = index
        .get_by_stable_id(7)
        .expect("child fragment should be indexed by render-handle fallback stable id");

    assert_eq!(fragment.parent_stable_id, Some(42));
}

#[test]
fn graph_fragment_index_only_indexes_cells_with_scalar_source() {
    let table_key = GraphNodeKey {
        stable_id: 1,
        path: vec![],
    };
    let scalar_source =
        treease_core::operators::TreeNode::scalar(treease_core::operators::SemType::Str, "ok");
    let object_source = treease_core::operators::TreeNode {
        kind: treease_core::operators::NodeKind::Mapping,
        ..treease_core::operators::TreeNode::default()
    };

    let model = BuilderGraphModel::from_parts(
        vec![GraphNode {
            render_handle: 1,
            stable_id: 1,
            key: table_key,
            kind: GraphKind::Table,
            depth: 0,
            x: 0,
            y: 0,
            width: 120,
            height: 40,
            box_args: Default::default(),
            path: vec![],
            meta: Default::default(),
            rows: vec![],
            table: Some(treease_core::core::graph_builder::GraphTable {
                columns: vec![
                    Default::default(),
                    treease_core::core::graph_builder::GraphCell {
                        text: "name".to_string(),
                        ..Default::default()
                    },
                    treease_core::core::graph_builder::GraphCell {
                        text: "meta".to_string(),
                        ..Default::default()
                    },
                    treease_core::core::graph_builder::GraphCell {
                        text: "missing".to_string(),
                        ..Default::default()
                    },
                ],
                rows: vec![treease_core::core::graph_builder::GraphRow {
                    index: 0,
                    key: Default::default(),
                    value: treease_core::core::graph_builder::GraphCell {
                        text: "ok".to_string(),
                        path: vec![
                            treease_core::core::PathSeg::Index(0),
                            treease_core::core::PathSeg::Key("name".to_string()),
                        ],
                        value: "ok".to_string(),
                        editable: true,
                        source: Some(
                            &scalar_source as *const treease_core::operators::TreeNode as usize,
                        ),
                        ..Default::default()
                    },
                    cells: vec![
                        Default::default(),
                        treease_core::core::graph_builder::GraphCell {
                            text: "ok".to_string(),
                            path: vec![
                                treease_core::core::PathSeg::Index(0),
                                treease_core::core::PathSeg::Key("name".to_string()),
                            ],
                            value: "ok".to_string(),
                            editable: true,
                            source: Some(
                                &scalar_source as *const treease_core::operators::TreeNode as usize,
                            ),
                            ..Default::default()
                        },
                        treease_core::core::graph_builder::GraphCell {
                            text: "{1}".to_string(),
                            path: vec![
                                treease_core::core::PathSeg::Index(0),
                                treease_core::core::PathSeg::Key("meta".to_string()),
                            ],
                            value: String::new(),
                            editable: true,
                            source: Some(
                                &object_source as *const treease_core::operators::TreeNode as usize,
                            ),
                            ..Default::default()
                        },
                        treease_core::core::graph_builder::GraphCell {
                            text: String::new(),
                            path: vec![
                                treease_core::core::PathSeg::Index(0),
                                treease_core::core::PathSeg::Key("missing".to_string()),
                            ],
                            editable: true,
                            source: None,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                column_widths: vec![10, 20, 20, 20],
                width: 70,
                total_height: 54,
                view_height: 54,
                header_height: 26,
                row_height: 28,
                key: "root".to_string(),
                count: 1,
                source: None,
            }),
            preorder_first: 1,
            preorder_last: 1,
            source: None,
        }],
        vec![],
    );

    let index = GraphFragmentIndex::build(&model);
    assert_eq!(index.indexed_table_cell_count(), 1);
    assert!(
        index
            .find_table_cell_by_path(&[
                treease_core::core::PathSeg::Index(0),
                treease_core::core::PathSeg::Key("name".to_string()),
            ])
            .is_some()
    );
    assert!(
        index
            .find_table_cell_by_path(&[
                treease_core::core::PathSeg::Index(0),
                treease_core::core::PathSeg::Key("meta".to_string()),
            ])
            .is_none()
    );
    assert!(
        index
            .find_table_cell_by_path(&[
                treease_core::core::PathSeg::Index(0),
                treease_core::core::PathSeg::Key("missing".to_string()),
            ])
            .is_none()
    );
}

#[test]
fn graph_fragment_index_bounds_subtree_bottom_for_cyclic_edges() {
    let node1_key = GraphNodeKey {
        stable_id: 1,
        path: vec![treease_core::core::PathSeg::Key("a".to_string())],
    };
    let node2_key = GraphNodeKey {
        stable_id: 2,
        path: vec![treease_core::core::PathSeg::Key("b".to_string())],
    };
    let model = BuilderGraphModel::from_parts(
        vec![
            GraphNode {
                render_handle: 1,
                stable_id: 1,
                key: node1_key.clone(),
                kind: GraphKind::Scalar,
                depth: 0,
                x: 0,
                y: 0,
                width: 10,
                height: 10,
                box_args: Default::default(),
                path: node1_key.path.clone(),
                meta: Default::default(),
                rows: vec![],
                table: None,
                preorder_first: 1,
                preorder_last: 1,
                source: None,
            },
            GraphNode {
                render_handle: 2,
                stable_id: 2,
                key: node2_key.clone(),
                kind: GraphKind::Scalar,
                depth: 1,
                x: 0,
                y: 20,
                width: 10,
                height: 10,
                box_args: Default::default(),
                path: node2_key.path.clone(),
                meta: Default::default(),
                rows: vec![],
                table: None,
                preorder_first: 2,
                preorder_last: 2,
                source: None,
            },
        ],
        vec![
            GraphEdge {
                from_render_handle: 1,
                from_key: node1_key.clone(),
                from_row: -1,
                to_render_handle: 2,
                to_key: node2_key.clone(),
                to_row: -1,
                bezier_args: Default::default(),
            },
            GraphEdge {
                from_render_handle: 2,
                from_key: node2_key,
                from_row: -1,
                to_render_handle: 1,
                to_key: node1_key,
                to_row: -1,
                bezier_args: Default::default(),
            },
        ],
    );

    let index = GraphFragmentIndex::build(&model);
    assert_eq!(index.get_by_stable_id(1).unwrap().bottom, 30);
    assert_eq!(index.get_by_stable_id(2).unwrap().bottom, 30);
}
