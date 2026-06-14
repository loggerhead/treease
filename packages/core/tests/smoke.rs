use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use treease_core::core::graph_model::GraphKind;
use treease_core::core::{
    BoxArgs, CodecState, Context, CoreError, Diagnostics, Encoder, FormatError, GraphCell,
    GraphModel, GraphNode, GraphNodeKey, NodeId, Printer, RegistryHandle, SemType, SystemError,
    TreeNode, TreeNodeKind, TreeStore, VecPrinterWriter, build_graph_delta, ensure_map, ensure_seq,
    ensure_seq_index, format_from_string, format_string_from_filename, get_or_create_map_value,
    path_seg_key,
};

struct IdEncoder;

impl Encoder for IdEncoder {
    fn encode(
        &self,
        _ctx: &mut Context,
        node: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        write!(writer, "node:{}\n", node.0)?;
        Ok(())
    }
}

struct NulEncoder;

impl Encoder for NulEncoder {
    fn encode(
        &self,
        _ctx: &mut Context,
        _node: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        writer.write_all(b"test\0value\n")?;
        Ok(())
    }
}

#[test]
fn format_lookup_matches_formal_name_and_alias() {
    assert_eq!(format_from_string("json").unwrap().formal_name, "json");
    assert_eq!(format_from_string("yml").unwrap().formal_name, "yaml");
    assert_eq!(format_string_from_filename("input.py"), "python");
    assert!(matches!(
        format_from_string(""),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));

    // LangSpec direct access
    assert!(treease_core::core::YAML_SPEC.matches_name("yml"));
    assert!(treease_core::core::YAML_SPEC.matches_extension("YAML"));
    assert_eq!(
        treease_core::core::lang_from_name("j").unwrap().name,
        "json"
    );
}

#[test]
fn format_filename_lookup_maps_known_extensions_and_keeps_unknowns() {
    assert_eq!(format_string_from_filename("a.yml"), "yaml");
    assert_eq!(format_string_from_filename("a.py"), "python");
    assert_eq!(format_string_from_filename("a.js"), "javascript");
    assert_eq!(format_string_from_filename("a.abc"), "abc");
    assert_eq!(format_string_from_filename("README"), "json");
}

#[test]
fn context_child_clones_views_without_sharing_node_list() {
    let diagnostics = Rc::new(RefCell::new(Diagnostics::default()));
    let mut ctx = Context::from_matching_nodes(RegistryHandle::default(), vec![NodeId(1)])
        .with_diagnostics(Some(diagnostics.clone()));
    ctx.set_variable("$x", vec![NodeId(2)]);
    ctx.ensure_codec_state().remember_original(NodeId(1), "one");

    let mut child = ctx.child_context(vec![NodeId(3)]).unwrap();
    child.append_matching_node(NodeId(4));
    child
        .diagnostics
        .as_ref()
        .unwrap()
        .borrow_mut()
        .push("child");

    assert_eq!(ctx.matching_nodes, vec![NodeId(1)]);
    assert_eq!(child.matching_nodes, vec![NodeId(3), NodeId(4)]);
    assert_eq!(child.get_variable("$x"), Some(&vec![NodeId(2)]));
    assert_eq!(diagnostics.borrow().messages(), &["child".to_string()]);
}

#[test]
fn context_child_without_variables_keeps_empty_variable_map() {
    let original = Context::empty(RegistryHandle::default());

    let child = original.child_context(vec![NodeId(3)]).unwrap();

    assert!(child.variables.is_empty());
}

#[test]
fn context_readonly_and_writable_clones_toggle_auto_create_flag() {
    let ctx = Context::from_matching_nodes(RegistryHandle::default(), vec![NodeId(1)]);

    let readonly = ctx.read_only_clone().unwrap();
    let writable = readonly.writable_clone().unwrap();

    assert!(readonly.dont_auto_create);
    assert!(!writable.dont_auto_create);
}

#[test]
fn codec_state_remembers_original_text_by_node_id() {
    let mut state = CodecState::new();
    state.remember_original(NodeId(42), "raw");

    assert_eq!(state.original_for(NodeId(42)), Some("raw"));
    assert_eq!(state.original_for(NodeId(7)), None);
}

#[test]
fn printer_writes_encoded_nodes_and_nul_separator() {
    let mut ctx = Context::empty(RegistryHandle::default());
    let store = TreeStore::new();
    let writer = VecPrinterWriter::new();
    let mut printer = Printer::new(IdEncoder, writer);
    printer.set_nul_sep_output(true);

    printer
        .print_results(&mut ctx, &store, &[NodeId(1), NodeId(2)])
        .unwrap();

    assert!(printer.printed_anything());
    let writer = printer.into_writer();
    assert_eq!(writer.as_slice(), b"node:1\0node:2\0");
}

#[test]
fn printer_remove_last_eol_strips_single_trailing_line_ending() {
    let mut buf = b"line1\r\nline2\r\n".to_vec();

    Printer::<IdEncoder, VecPrinterWriter>::remove_last_eol(&mut buf);

    assert_eq!(buf, b"line1\r\nline2");
}

#[test]
fn printer_writes_appendix_when_no_nodes_match() {
    let mut ctx = Context::empty(RegistryHandle::default());
    let store = TreeStore::new();
    let writer = VecPrinterWriter::new();
    let mut printer = Printer::new(IdEncoder, writer);
    printer.set_appendix(Some(b"appendix content".to_vec()));

    printer.print_results(&mut ctx, &store, &[]).unwrap();

    let writer = printer.into_writer();
    assert_eq!(writer.as_slice(), b"appendix content");
}

#[test]
fn printer_rejects_nul_bytes_in_nul_separated_output() {
    let mut ctx = Context::empty(RegistryHandle::default());
    let store = TreeStore::new();
    let writer = VecPrinterWriter::new();
    let mut printer = Printer::new(NulEncoder, writer);
    printer.set_nul_sep_output(true);

    let error = printer
        .print_results(&mut ctx, &store, &[NodeId(1)])
        .unwrap_err();

    assert_eq!(
        error,
        CoreError::System(SystemError::NulInNulSeparatedOutput)
    );
}

#[test]
fn tree_store_uses_node_ids_for_parent_and_sequence_paths() {
    let mut store = TreeStore::new();
    let mut root = TreeNode::default();
    root.kind = TreeNodeKind::Sequence;
    root.set_sem_type(SemType::Seq);
    root.set_document(3);
    root.set_filename("input.json");
    let root_id = store.add(root);

    let child_id = ensure_seq_index(&mut store, root_id, 0).unwrap();

    let child = store.get(child_id).unwrap();
    assert_eq!(child.parent, Some(root_id));
    assert_eq!(child.sequence_index, Some(0));
    assert_eq!(store.document_for(child_id).unwrap(), 3);
    assert_eq!(store.filename_for(child_id).unwrap(), "input.json");
    assert_eq!(store.nice_path_for(child_id).unwrap(), "[0]");
}

#[test]
fn tree_store_builds_nested_paths_for_dotted_map_keys_and_sequence_items() {
    let mut store = TreeStore::new();
    let mut root = TreeNode::default();
    root.kind = TreeNodeKind::Mapping;
    let root_id = store.add(root);
    let (_, value_id) = store
        .add_key_value_child(
            root_id,
            TreeNode::scalar(SemType::Str, "a.b"),
            TreeNode {
                kind: TreeNodeKind::Sequence,
                ..TreeNode::default()
            },
        )
        .unwrap();

    let item_id = ensure_seq_index(&mut store, value_id, 0).unwrap();

    assert_eq!(
        store.path_for(item_id).unwrap(),
        vec![
            treease_core::core::ParsedKey::Str("a.b".to_string()),
            treease_core::core::ParsedKey::Int(0),
        ]
    );
    assert_eq!(store.nice_path_for(item_id).unwrap(), "a.b[0]");
}

#[test]
fn tree_ops_ensure_map_and_sequence_clear_scalar_value() {
    let mut store = TreeStore::new();
    let map_id = store.add(TreeNode::scalar(SemType::Str, "value"));
    let seq_id = store.add(TreeNode::scalar(SemType::Str, "value"));

    ensure_map(&mut store, map_id).unwrap();
    ensure_seq(&mut store, seq_id).unwrap();

    assert_eq!(store.get(map_id).unwrap().kind, TreeNodeKind::Mapping);
    assert_eq!(store.get(seq_id).unwrap().kind, TreeNodeKind::Sequence);
    assert_eq!(store.get(map_id).unwrap().value, "");
    assert_eq!(store.get(seq_id).unwrap().value, "");
}

#[test]
fn tree_ops_create_map_value_with_key_node_id() {
    let mut store = TreeStore::new();
    let mut root = TreeNode::default();
    root.kind = TreeNodeKind::Mapping;
    root.set_sem_type(SemType::Map);
    let root_id = store.add(root);

    let value_id = get_or_create_map_value(&mut store, root_id, "name").unwrap();

    let value = store.get(value_id).unwrap();
    let key_id = value.key.unwrap();
    assert_eq!(value.parent, Some(root_id));
    assert_eq!(store.get(key_id).unwrap().value, "name");
    assert_eq!(
        store.path_for(value_id).unwrap(),
        vec![treease_core::core::ParsedKey::Str("name".to_string())]
    );
}

#[test]
fn graph_delta_tracks_added_updated_and_removed_nodes_by_stable_id() {
    let source = NodeId(0);
    let path = [path_seg_key("name")];
    let rows = [];
    let cell = GraphCell {
        text: "name",
        value: "Ada",
        path: &path,
        source: Some(source),
        ..GraphCell::default()
    };
    let key = GraphNodeKey {
        kind: GraphKind::Scalar,
        path: &path,
        path_key: "$.name",
        stable_id: 10,
        stable_id_text: "10",
    };
    let old_node = graph_node(1, 10, key, cell, &rows, source, 100);
    let updated_node = graph_node(1, 10, key, cell, &rows, source, 120);
    let added_node = graph_node(2, 20, key, cell, &rows, source, 100);
    let old_model = GraphModel {
        nodes: vec![old_node],
        edges: vec![],
    };
    let new_model = GraphModel {
        nodes: vec![updated_node, added_node],
        edges: vec![],
    };

    let delta = build_graph_delta(Some(&old_model), &new_model);

    assert_eq!(delta.nodes_added, vec![added_node]);
    assert_eq!(delta.nodes_updated, vec![updated_node]);
    assert!(delta.nodes_removed.is_empty());
}

#[test]
fn graph_delta_marks_first_model_as_clear_with_added_nodes_and_edges() {
    let source = NodeId(0);
    let path = [path_seg_key("name")];
    let rows = [];
    let cell = GraphCell {
        text: "name",
        value: "Ada",
        path: &path,
        source: Some(source),
        ..GraphCell::default()
    };
    let key = GraphNodeKey {
        kind: GraphKind::Scalar,
        path: &path,
        path_key: "$.name",
        stable_id: 10,
        stable_id_text: "10",
    };
    let node = graph_node(1, 10, key, cell, &rows, source, 100);
    let model = GraphModel {
        nodes: vec![node],
        edges: vec![],
    };

    let delta = build_graph_delta(None, &model);

    assert!(delta.clear);
    assert_eq!(delta.nodes_added, vec![node]);
}

#[test]
fn graph_delta_tracks_removed_node_render_handle() {
    let source = NodeId(0);
    let path = [path_seg_key("name")];
    let rows = [];
    let cell = GraphCell {
        text: "name",
        value: "Ada",
        path: &path,
        source: Some(source),
        ..GraphCell::default()
    };
    let key = GraphNodeKey {
        kind: GraphKind::Scalar,
        path: &path,
        path_key: "$.name",
        stable_id: 10,
        stable_id_text: "10",
    };
    let old_node = graph_node(7, 10, key, cell, &rows, source, 100);
    let old_model = GraphModel {
        nodes: vec![old_node],
        edges: vec![],
    };
    let new_model = GraphModel {
        nodes: vec![],
        edges: vec![],
    };

    let delta = build_graph_delta(Some(&old_model), &new_model);

    assert_eq!(delta.nodes_removed, vec![7]);
}

fn graph_node<'a>(
    render_handle: u32,
    stable_id: u64,
    key: GraphNodeKey<'a>,
    meta: GraphCell<'a>,
    rows: &'a [treease_core::core::GraphRow<'a>],
    source: NodeId,
    width: i32,
) -> GraphNode<'a> {
    GraphNode {
        render_handle,
        stable_id,
        key,
        kind: GraphKind::Scalar,
        depth: 0,
        x: 0,
        y: 0,
        width,
        height: 20,
        box_args: BoxArgs::default(),
        path: key.path,
        meta,
        rows,
        table: None,
        source,
        preorder_first: 0,
        preorder_last: 0,
    }
}
