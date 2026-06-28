use crate::operators::compat::{NodeId as CompatNodeId, SemType as CompatSemType};
use crate::operators::{NodeKind as CompatNodeKind, TreeNode as CompatTreeNode};

use super::graph_builder::{
    BuilderConfig, GraphBuilder, GraphCell, GraphLanguage, GraphNode, GraphRow, PathSeg,
};
use super::{NodeId, TreeNodeKind, TreeStore};

pub(crate) struct NodeShapeBuilder<'a> {
    config: &'a BuilderConfig,
    language: GraphLanguage,
}

impl<'a> NodeShapeBuilder<'a> {
    pub(crate) fn new(config: &'a BuilderConfig, language: GraphLanguage) -> Self {
        Self { config, language }
    }

    pub(crate) fn build_node_from_store(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        path: &[PathSeg],
        depth: u32,
        render_handle: u32,
    ) -> Option<GraphNode> {
        let compat = compat_tree_from_store(store, node_id)?;
        let layout_builder = GraphBuilder::new(self.config.clone(), self.language);
        let mut node = layout_builder.build_node_only(&compat, depth, path, render_handle);
        node.preorder_first = render_handle;
        node.preorder_last = render_handle;
        layout_builder.apply_node_bounds_to(&mut node);
        Some(node)
    }

    pub(crate) fn rebuild_node_from_store(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        current: &GraphNode,
    ) -> Option<GraphNode> {
        let mut rebuilt = self.build_node_from_store(
            store,
            node_id,
            &current.path,
            current.depth,
            current.render_handle,
        )?;
        rebuilt.x = current.x;
        rebuilt.y = current.y;
        rebuilt.box_args.x = current.box_args.x;
        rebuilt.box_args.y = current.box_args.y;
        rebuilt.preorder_first = current.preorder_first;
        rebuilt.preorder_last = current.preorder_last;
        let layout_builder = GraphBuilder::new(self.config.clone(), self.language);
        layout_builder.apply_node_bounds_to(&mut rebuilt);
        Some(rebuilt)
    }

    pub(crate) fn install_object_rows(&self, node: &mut GraphNode, rows: Vec<GraphRow>) {
        let layout_builder = GraphBuilder::new(self.config.clone(), self.language);
        layout_builder.install_object_rows(node, rows);
    }
}

pub(crate) fn build_aligned_row_from_mapping(
    store: &TreeStore,
    mapping_id: NodeId,
    parent_path: &[PathSeg],
    parent_columns: &[GraphCell],
    row_index: i32,
) -> Option<GraphRow> {
    let mapping = store.get(mapping_id)?;
    if mapping.kind != TreeNodeKind::Mapping {
        return None;
    }

    let mut item_path = parent_path.to_vec();
    item_path.push(PathSeg::Index(row_index as usize));

    let index_text = row_index.to_string();
    let index_cell = GraphCell {
        text: index_text.clone(),
        path: item_path.clone(),
        value: index_text,
        editable: false,
        source: Some(mapping as *const _ as usize),
        ..GraphCell::default()
    };

    let mut cells = Vec::with_capacity(parent_columns.len().max(1));
    cells.push(index_cell);

    for column in parent_columns.iter().skip(1) {
        let mut cell_path = item_path.clone();
        cell_path.push(PathSeg::Key(column.text.clone()));
        let mut value_text = String::new();
        let mut sem_type = None;
        let mut source = None;
        let mut found = false;
        let mut index = 0;
        while index + 1 < mapping.content.len() {
            let key_id = mapping.content[index];
            let value_id = mapping.content[index + 1];
            if let Some(key) = store.get(key_id) {
                if key.value == column.text {
                    if let Some(value) = store.get(value_id) {
                        value_text = inline_row_text(value);
                        sem_type = value.resolved_sem_type().map(|sem| sem.tag().to_owned());
                        source = Some(value as *const _ as usize);
                    }
                    found = true;
                    break;
                }
            }
            index += 2;
        }
        let cell = if found {
            GraphCell {
                text: value_text.clone(),
                sem_type,
                path: cell_path,
                value: value_text,
                editable: true,
                source,
                ..GraphCell::default()
            }
        } else {
            GraphCell {
                text: "miss".to_string(),
                sem_type: column.sem_type.clone(),
                is_missing: true,
                value: "miss".to_string(),
                editable: false,
                ..GraphCell::default()
            }
        };
        cells.push(cell);
    }

    let key = cells.first().cloned().unwrap_or_default();
    let value = cells.get(1).cloned().unwrap_or_default();
    Some(GraphRow {
        index: row_index,
        key,
        value,
        cells,
        ..GraphRow::default()
    })
}

pub(crate) fn build_headerless_inline_row(
    store: &TreeStore,
    row_id: NodeId,
    parent_path: &[PathSeg],
) -> Option<GraphRow> {
    let node = store.get(row_id)?;
    let row_index = usize::try_from(node.sequence_index?).ok()?;
    let mut item_path = parent_path.to_vec();
    item_path.push(PathSeg::Index(row_index));

    let index_text = row_index.to_string();
    let key = GraphCell {
        text: index_text.clone(),
        path: item_path.clone(),
        value: index_text,
        editable: false,
        source: Some(node as *const _ as usize),
        ..GraphCell::default()
    };
    let value_text = inline_row_text(node);
    let value = GraphCell {
        text: value_text,
        sem_type: node.resolved_sem_type().map(|sem| sem.tag().to_owned()),
        path: item_path,
        value: node.value.clone(),
        editable: true,
        source: Some(node as *const _ as usize),
        ..GraphCell::default()
    };
    Some(GraphRow {
        index: row_index as i32,
        key: key.clone(),
        value: value.clone(),
        cells: vec![key, value],
        ..GraphRow::default()
    })
}

pub(crate) fn build_object_rows_from_mapping(
    store: &TreeStore,
    mapping_id: NodeId,
    parent_path: &[PathSeg],
) -> Vec<GraphRow> {
    let Some(mapping) = store.get(mapping_id) else {
        return Vec::new();
    };
    if mapping.kind != TreeNodeKind::Mapping {
        return Vec::new();
    }
    let mut rows = Vec::with_capacity(mapping.content.len() / 2);
    let mut index = 0;
    let mut row_index = 0i32;
    while index + 1 < mapping.content.len() {
        let key_id = mapping.content[index];
        let value_id = mapping.content[index + 1];
        index += 2;
        let (Some(key_node), Some(value_node)) = (store.get(key_id), store.get(value_id)) else {
            continue;
        };
        let mut cell_path = parent_path.to_vec();
        cell_path.push(PathSeg::Key(key_node.value.clone()));

        let key_cell = GraphCell {
            text: key_node.value.clone(),
            sem_type: key_node.resolved_sem_type().map(|sem| sem.tag().to_owned()),
            path: cell_path.clone(),
            value: key_node.value.clone(),
            editable: true,
            source: Some(key_node as *const _ as usize),
            ..GraphCell::default()
        };
        let value_text = inline_row_text(value_node);
        let value_cell = GraphCell {
            text: value_text,
            sem_type: value_node
                .resolved_sem_type()
                .map(|sem| sem.tag().to_owned()),
            path: cell_path,
            value: value_node.value.clone(),
            editable: true,
            source: Some(value_node as *const _ as usize),
            ..GraphCell::default()
        };
        rows.push(GraphRow {
            index: row_index,
            key: key_cell.clone(),
            value: value_cell.clone(),
            cells: vec![key_cell, value_cell],
            ..GraphRow::default()
        });
        row_index += 1;
    }
    rows
}

pub(crate) fn header_table_row_has_new_column(
    table_columns: &[GraphCell],
    store: &TreeStore,
    row_id: NodeId,
) -> bool {
    let Some(row) = store.get(row_id) else {
        return false;
    };
    if row.kind != TreeNodeKind::Mapping {
        return false;
    }
    let mut index = 0;
    while index + 1 < row.content.len() {
        let key_id = row.content[index];
        if let Some(key) = store.get(key_id) {
            let known = table_columns
                .iter()
                .skip(1)
                .any(|column| column.text == key.value);
            if !known {
                return true;
            }
        }
        index += 2;
    }
    false
}

pub(crate) fn inline_row_text(node: &crate::core::TreeNode) -> String {
    match node.kind {
        TreeNodeKind::Scalar => node.value.clone(),
        TreeNodeKind::Mapping => {
            let count = node.content.len() / 2;
            if count == 0 {
                "{}".to_owned()
            } else {
                format!("{{{count}}}")
            }
        }
        TreeNodeKind::Sequence => {
            if !node.sequence_closed {
                "[?]".to_owned()
            } else {
                let count = node.content.len();
                if count == 0 {
                    "[]".to_owned()
                } else {
                    format!("[{count}]")
                }
            }
        }
        TreeNodeKind::Alias => "*".to_owned(),
        TreeNodeKind::Unknown => String::new(),
    }
}

fn compat_tree_from_store(store: &TreeStore, id: NodeId) -> Option<CompatTreeNode> {
    let node = store.get(id)?;
    let mut content = Vec::with_capacity(node.content.len());
    for &child in &node.content {
        content.push(compat_tree_from_store(store, child)?);
    }
    Some(CompatTreeNode {
        kind: match node.kind {
            TreeNodeKind::Scalar => CompatNodeKind::Scalar,
            TreeNodeKind::Mapping => CompatNodeKind::Mapping,
            TreeNodeKind::Sequence => CompatNodeKind::Sequence,
            TreeNodeKind::Alias => CompatNodeKind::Alias,
            TreeNodeKind::Unknown => CompatNodeKind::Unknown,
        },
        sequence_closed: node.sequence_closed,
        sem_type: node.sem_type.map(CompatSemType::from),
        tag: node.tag.clone(),
        value: node.value.clone(),
        start_byte: node.start_byte,
        end_byte: node.end_byte,
        content,
        leading_content: node.leading_content.clone(),
        parent: node.parent.map(|id| CompatNodeId(id.0)),
        key: node.key.map(|id| CompatNodeId(id.0)),
        is_map_key: node.is_map_key,
        sequence_index: node.sequence_index,
        alias: node.alias.map(|id| CompatNodeId(id.0)),
        anchor: node.anchor.clone(),
        head_comment: node.head_comment.clone(),
        line_comment: node.line_comment.clone(),
        foot_comment: node.foot_comment.clone(),
        document: node.document,
        filename: node.filename.clone(),
        line: node.line,
        column: node.column,
        file_index: node.file_index,
        encode_separate: node.encode_separate,
        evaluate_together: node.evaluate_together,
    })
}
