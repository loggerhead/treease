use crate::operators::{NodeKind, TreeNode};

use super::shared::{append_formatted_path_seg, set_cell_bounds, set_text_bounds};
use super::{GraphBuilder, GraphCell, PathSeg};

pub(super) fn make_meta_cell(
    builder: &GraphBuilder,
    node: &TreeNode,
    path: &[PathSeg],
    x: i32,
    y: i32,
    inner_width: i32,
) -> GraphCell {
    let full_path = format_path(path);
    let display_path = format_meta_path(
        builder,
        path,
        &full_path,
        node_has_child_nodes(builder, node),
    );
    let mut meta = GraphCell {
        text: display_path,
        sem_type: node.resolved_sem_type().map(|st| st.tag().to_owned()),
        path: path.to_vec(),
        value: full_path,
        editable: false,
        source: Some(node as *const TreeNode as usize),
        ..GraphCell::default()
    };
    set_cell_bounds(
        &mut meta,
        x,
        y - builder.config.row_height,
        inner_width,
        builder.config.row_height,
    );
    set_text_bounds(
        &mut meta,
        x + builder.config.row_padding_x,
        y - builder.config.row_height,
        inner_width - builder.config.row_padding_x * 2,
        builder.config.row_height,
    );
    meta
}

pub(super) fn format_path(path: &[PathSeg]) -> String {
    if path.is_empty() {
        return "$".to_string();
    }

    let mut out = String::new();
    for (idx, seg) in path.iter().enumerate() {
        append_formatted_path_seg(&mut out, seg, idx > 0);
    }
    out
}

pub(super) fn format_meta_path(
    builder: &GraphBuilder,
    path: &[PathSeg],
    full_path: &str,
    has_children: bool,
) -> String {
    if path.is_empty() {
        return full_path.to_string();
    }
    if !has_children {
        return full_path.to_string();
    }
    let min_segments = builder.config.meta_path_min_segments.max(0) as usize;
    let min_chars = builder.config.meta_path_min_chars.max(0) as usize;
    let keep_tail = builder.config.meta_path_keep_tail_segments.max(0).max(1) as usize;
    if path.len() <= min_segments {
        return full_path.to_string();
    }
    if full_path.len() <= min_chars {
        return full_path.to_string();
    }
    if keep_tail >= path.len() {
        return full_path.to_string();
    }
    let mut buf = String::from("...");
    let tail_start = path.len() - keep_tail;
    for (idx, seg) in path.iter().enumerate().skip(tail_start) {
        let is_first = idx == tail_start;
        append_meta_path_seg(&mut buf, seg, is_first);
    }
    buf
}

fn append_meta_path_seg(buf: &mut String, seg: &PathSeg, is_first: bool) {
    append_formatted_path_seg(buf, seg, !is_first);
}

pub(super) fn cell_text_for_node(node: &TreeNode) -> &str {
    match node.kind {
        NodeKind::Scalar => &node.value,
        NodeKind::Mapping => "{}",
        NodeKind::Sequence => "[]",
        NodeKind::Alias => "*",
        NodeKind::Unknown => "",
    }
}

pub(super) fn display_text_for_node(_builder: &GraphBuilder, node: &TreeNode) -> String {
    match node.kind {
        NodeKind::Mapping | NodeKind::Sequence => collection_summary_text(node),
        _ => cell_text_for_node(node).to_string(),
    }
}

pub(super) fn collection_summary_text(node: &TreeNode) -> String {
    if node.kind == NodeKind::Mapping {
        let count = (node.content.len() / 2) as i32;
        if count == 0 {
            return "{}".to_string();
        }
        return format!("{{{}}}", count);
    }
    if node.kind == NodeKind::Sequence {
        if !node.sequence_closed {
            return "[?]".to_string();
        }
        let count = node.content.len() as i32;
        if count == 0 {
            return "[]".to_string();
        }
        return format!("[{}]", count);
    }
    String::new()
}

fn node_has_child_nodes(builder: &GraphBuilder, node: &TreeNode) -> bool {
    super::shared::node_has_child_nodes(builder, node)
}
