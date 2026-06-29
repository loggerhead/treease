use crate::core::SemType;
use crate::operators::{NodeKind, TreeNode};

use super::{BoxArgs, CellBounds, GraphBuilder, GraphCell, GraphNode, GraphRow, PathSeg};

pub(super) fn set_bounds(bounds: &mut CellBounds, x: i32, y: i32, width: i32, height: i32) {
    *bounds = CellBounds {
        x,
        y,
        width: width.max(0),
        height: height.max(0),
    };
}

pub(super) fn set_cell_bounds(cell: &mut GraphCell, x: i32, y: i32, width: i32, height: i32) {
    set_bounds(&mut cell.bounds, x, y, width, height);
    cell.box_args = BoxArgs {
        x: cell.bounds.x,
        y: cell.bounds.y,
        width: cell.bounds.width,
        height: cell.bounds.height,
        corner_radius: 0,
    };
}

pub(super) fn set_text_bounds(cell: &mut GraphCell, x: i32, y: i32, width: i32, height: i32) {
    set_bounds(&mut cell.text_bounds, x, y, width, height);
}

pub(super) fn value_node_builds_child(node: &TreeNode) -> bool {
    if node.kind != NodeKind::Mapping && node.kind != NodeKind::Sequence {
        return false;
    }
    !node.content.is_empty()
}

pub(super) fn sequence_has_header_table(node: &TreeNode) -> bool {
    if node.content.is_empty() {
        return false;
    }
    node.content
        .first()
        .is_some_and(|c| c.kind == NodeKind::Mapping)
}

pub(super) fn sequence_table_children_can_expand(builder: &GraphBuilder, node: &TreeNode) -> bool {
    if node.kind != NodeKind::Sequence || !node.sequence_closed {
        return false;
    }
    sequence_table_total_height(builder, node) <= builder.config.table_max_height
}

fn sequence_table_total_height(builder: &GraphBuilder, node: &TreeNode) -> i32 {
    let row_count = node.content.len() as i32;
    let base_rows = row_count.max(1);
    if sequence_has_header_table(node) {
        builder.config.table_header_height + builder.config.table_row_height * base_rows
    } else {
        builder.config.row_height * base_rows
    }
}

pub(super) fn header_table_needs_fallback_value_column(node: &TreeNode) -> bool {
    let mut has_mapping_key = false;
    for item in &node.content {
        if item.kind != NodeKind::Mapping {
            return true;
        }
        if !item.content.is_empty() {
            has_mapping_key = true;
        }
    }
    !has_mapping_key
}

pub(super) fn infer_header_table_column_sem_type(node: &TreeNode, key: &str) -> Option<String> {
    for item in &node.content {
        if item.kind != NodeKind::Mapping {
            continue;
        }
        let mut index = 0;
        while index + 1 < item.content.len() {
            let key_node = &item.content[index];
            if key_node.value == key {
                return Some(SemType::Str.tag().to_owned());
            }
            index += 2;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::infer_header_table_column_sem_type;
    use crate::core::SemType as CoreSemType;
    use crate::operators::{NodeKind, SemType, TreeNode};

    fn mapping_pair(key: &str, value: TreeNode) -> TreeNode {
        let mut map = TreeNode {
            kind: NodeKind::Mapping,
            sem_type: Some(SemType::Map),
            tag: CoreSemType::Map.to_string(),
            ..TreeNode::default()
        };
        map.content.push(TreeNode::scalar(SemType::Str, key));
        map.content.push(value);
        map
    }

    #[test]
    fn header_table_columns_keep_string_semantics() {
        let mut table = TreeNode {
            kind: NodeKind::Sequence,
            sem_type: Some(SemType::Seq),
            tag: CoreSemType::Seq.to_string(),
            ..TreeNode::default()
        };
        table
            .content
            .push(mapping_pair("h1", TreeNode::scalar(SemType::Int, "11")));

        assert_eq!(
            infer_header_table_column_sem_type(&table, "h1"),
            Some(CoreSemType::Str.tag().to_owned())
        );
    }
}

pub(super) fn set_row_bounds(row: &mut GraphRow, x: i32, y: i32, width: i32, height: i32) {
    set_bounds(&mut row.bounds, x, y, width, height);
    row.box_args = BoxArgs {
        x: row.bounds.x,
        y: row.bounds.y,
        width: row.bounds.width,
        height: row.bounds.height,
        corner_radius: 0,
    };
}

pub(super) fn set_row_abs_bounds(row: &mut GraphRow, x: i32, y: i32, width: i32, height: i32) {
    set_bounds(&mut row.abs_bounds, x, y, width, height);
}

pub(super) fn set_row_cell_bounds(row: &mut GraphRow, x: i32, y: i32, width: i32, height: i32) {
    set_bounds(&mut row.cell_bounds, x, y, width, height);
    row.cell_box_args = BoxArgs {
        x: row.cell_bounds.x,
        y: row.cell_bounds.y,
        width: row.cell_bounds.width,
        height: row.cell_bounds.height,
        corner_radius: 0,
    };
}

pub(super) fn append_path(base: &[PathSeg], seg: PathSeg) -> Vec<PathSeg> {
    let mut next = base.to_vec();
    next.push(seg);
    next
}

pub(super) fn append_path_index(buf: &mut String, index: i32) {
    buf.push('[');
    buf.push_str(&index.to_string());
    buf.push(']');
}

pub(super) fn append_formatted_path_seg(buf: &mut String, seg: &PathSeg, prepend_dot: bool) {
    match seg {
        PathSeg::Key(key) => {
            if prepend_dot {
                buf.push('.');
            }
            buf.push_str(key);
        }
        PathSeg::Index(index) => {
            append_path_index(buf, *index as i32);
        }
    }
}

pub(super) fn find_node_in_list<'a>(
    nodes: &'a [GraphNode],
    render_handle: u32,
) -> Option<&'a GraphNode> {
    nodes.iter().find(|n| n.render_handle == render_handle)
}

/// Check whether a node has any child nodes that would produce sub-graph nodes.
pub(super) fn node_has_child_nodes(builder: &GraphBuilder, node: &TreeNode) -> bool {
    match node.kind {
        NodeKind::Mapping => {
            let mut i = 0;
            while i + 1 < node.content.len() {
                let value_node = &node.content[i + 1];
                if value_node_builds_child(value_node) {
                    return true;
                }
                i += 2;
            }
            false
        }
        NodeKind::Sequence => {
            if !sequence_table_children_can_expand(builder, node) {
                return false;
            }
            if super::shared::sequence_has_header_table(node) {
                return builder.expand_table_children && table_has_child_nodes(node);
            }
            table_has_child_nodes(node)
        }
        _ => false,
    }
}

fn table_has_child_nodes(node: &TreeNode) -> bool {
    for item in &node.content {
        if value_node_builds_child(item) {
            return true;
        }
    }
    false
}
