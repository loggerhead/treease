use std::collections::BTreeSet;

use super::protocol::{
    DocumentAnchor, DocumentNodePreview, DocumentPathValue, DocumentSearchItem, ProjectionDelta,
    QueryKind, QueryResult, QueryTargetKind, SnapshotQuery,
};
use super::snapshot::{AnalysisBundle, DocumentSnapshot};
use crate::formats::DecodedDocument;
use crate::graph::graph_builder::PathSeg as GraphPathSeg;
use crate::graph::graph_projection_service;
use crate::language::SemType;
use crate::tree::tree_store::TreeStore;
use crate::tree::{
    NodeId, OwnedPathSeg, ParsedKey, TreeNode, TreeNodeKind, borrowed_tree_path,
    compute_path_span_for_document, compute_tree_path_segments_for_document,
    format_owned_tree_path, format_tree_path, parse_tree_path, path_seg_index, path_seg_key,
};

pub(crate) fn query_snapshot(snapshot: &DocumentSnapshot, query: &SnapshotQuery) -> QueryResult {
    let Some(analysis) = &snapshot.analysis else {
        return QueryResult::default();
    };
    let Some(document) = &analysis.document else {
        return QueryResult::default();
    };

    match query.kind {
        QueryKind::ResolvePath | QueryKind::ResolveHover => {
            let Some((start, end)) = query.span else {
                return QueryResult::default();
            };
            resolve_anchor_for_span(snapshot.snapshot_id, analysis, document, start, end)
                .map(|anchor| QueryResult {
                    anchors: vec![anchor],
                    ..Default::default()
                })
                .unwrap_or_default()
        }
        QueryKind::FindAnchors => query
            .path_pattern
            .as_deref()
            .and_then(|path| {
                resolve_anchor_for_path(
                    snapshot.snapshot_id,
                    analysis,
                    document,
                    path,
                    query.target.unwrap_or_default(),
                )
            })
            .map(|anchor| QueryResult {
                anchors: vec![anchor],
                ..Default::default()
            })
            .unwrap_or_default(),
        QueryKind::RootValueKind => QueryResult {
            root_value_kind: document.store.get(document.root).map(node_value_kind),
            ..Default::default()
        },
        QueryKind::NodePreview => query
            .path_pattern
            .as_deref()
            .and_then(|path| node_preview_for_path(document, path))
            .map(|node_preview| QueryResult {
                node_preview: Some(node_preview),
                ..Default::default()
            })
            .unwrap_or_default(),
        QueryKind::PathValue => query
            .path_pattern
            .as_deref()
            .and_then(|path| path_value_for_path(analysis, document, path))
            .map(|path_value| QueryResult {
                path_value: Some(path_value),
                ..Default::default()
            })
            .unwrap_or_default(),
        QueryKind::FieldLabels => QueryResult {
            field_labels: collect_field_labels(document),
            ..Default::default()
        },
        QueryKind::SearchIndex => QueryResult {
            search_items: collect_search_items(document),
            ..Default::default()
        },
    }
}

fn resolve_anchor_for_span(
    snapshot_id: super::protocol::SnapshotId,
    analysis: &AnalysisBundle,
    document: &DecodedDocument,
    start: u32,
    end: u32,
) -> Option<DocumentAnchor> {
    let line_column = analysis.line_index.offset_to_line_column(start as usize);
    let path = compute_tree_path_segments_for_document(
        &document.store,
        document.root,
        analysis.ts_tree.as_ref(),
        &analysis.diagnostics,
        &analysis.language,
        &analysis.source,
        &analysis.line_index,
        line_column.line,
        line_column.column,
    );
    if path.is_empty() {
        return None;
    }
    let borrowed_path = borrowed_tree_path(&path);
    let span = compute_path_span_for_document(
        &document.store,
        document.root,
        analysis.ts_tree.as_ref(),
        &analysis.diagnostics,
        &analysis.language,
        &analysis.source,
        &borrowed_path,
        false,
    );
    if span.start_byte < 0 || span.end_byte < span.start_byte {
        return None;
    }
    Some(DocumentAnchor {
        snapshot_id,
        path: format_owned_tree_path(&path),
        span_start: span.start_byte as u32,
        span_end: (span.end_byte as u32).max(end),
    })
}

fn resolve_anchor_for_path(
    snapshot_id: super::protocol::SnapshotId,
    analysis: &AnalysisBundle,
    document: &DecodedDocument,
    path_pattern: &str,
    target: QueryTargetKind,
) -> Option<DocumentAnchor> {
    let path = parse_tree_path(path_pattern)?;
    let borrowed_path = borrowed_tree_path(&path);
    let span = compute_path_span_for_document(
        &document.store,
        document.root,
        analysis.ts_tree.as_ref(),
        &analysis.diagnostics,
        &analysis.language,
        &analysis.source,
        &borrowed_path,
        matches!(target, QueryTargetKind::Key),
    );
    if span.start_byte < 0 || span.end_byte < span.start_byte {
        return None;
    }
    Some(DocumentAnchor {
        snapshot_id,
        path: format_owned_tree_path(&path),
        span_start: span.start_byte as u32,
        span_end: span.end_byte as u32,
    })
}

fn parsed_keys_from_snapshot_path(path: &str) -> Option<Vec<ParsedKey>> {
    parse_tree_path(path).map(|segments| {
        segments
            .into_iter()
            .map(|segment| match segment {
                OwnedPathSeg::Key(key) => ParsedKey::Str(key),
                OwnedPathSeg::Index(index) => ParsedKey::Int(i64::from(index)),
            })
            .collect()
    })
}

fn node_id_for_path(
    document: &DecodedDocument,
    path_pattern: &str,
    prefer_key: bool,
) -> Option<NodeId> {
    let path = parsed_keys_from_snapshot_path(path_pattern)?;
    if path.is_empty() {
        return Some(document.root);
    }
    document
        .store
        .find_descendant_by_path(document.root, &path, prefer_key)
        .ok()
        .flatten()
}

pub(crate) fn build_hover_subgraph_projection_for_snapshot(
    snapshot: &DocumentSnapshot,
    path: &str,
) -> Result<ProjectionDelta, &'static str> {
    let analysis = snapshot.analysis.as_ref().ok_or("no analysis")?;
    let document = analysis.document.as_ref().ok_or("no document")?;
    let path = parse_tree_path(path).ok_or("invalid path")?;
    let root = resolve_projection_node_for_path(&document.store, document.root, &path)
        .ok_or("path not found")?;
    let root_path = snapshot_projection_path_to_graph_path(&path);
    Ok(ProjectionDelta {
        clear: true,
        graph_data: Some(graph_projection_service::build_hover_subgraph_delta(
            &document.store,
            root,
            &analysis.language,
            &root_path,
        )),
        ..Default::default()
    })
}

fn resolve_projection_node_for_path(
    store: &crate::tree::TreeStore,
    root: NodeId,
    path: &[OwnedPathSeg],
) -> Option<NodeId> {
    let mut current = root;
    for segment in path {
        let node = store.get(current)?;
        current = match segment {
            OwnedPathSeg::Key(key) => {
                if node.kind != TreeNodeKind::Mapping {
                    return None;
                }
                node.content.chunks_exact(2).find_map(|pair| {
                    let key_node = store.get(pair[0])?;
                    (store.value_for(pair[0]).ok()? == key && key_node.is_map_key)
                        .then_some(pair[1])
                })?
            }
            OwnedPathSeg::Index(index) => {
                if node.kind != TreeNodeKind::Sequence || *index < 0 {
                    return None;
                }
                *node.content.get(*index as usize)?
            }
        };
    }
    Some(current)
}

fn snapshot_projection_path_to_graph_path(path: &[OwnedPathSeg]) -> Vec<GraphPathSeg> {
    path.iter()
        .map(|segment| match segment {
            OwnedPathSeg::Key(key) => GraphPathSeg::Key(key.clone()),
            OwnedPathSeg::Index(index) => GraphPathSeg::Index((*index).max(0) as usize),
        })
        .collect()
}

fn node_value_kind(node: &TreeNode) -> String {
    match node.kind {
        TreeNodeKind::Mapping => "object",
        TreeNodeKind::Sequence => "array",
        TreeNodeKind::Scalar | TreeNodeKind::Alias | TreeNodeKind::Unknown => {
            match node.resolved_sem_type() {
                Some(SemType::Str) => "string",
                Some(SemType::Int) => "int",
                Some(SemType::Float) => "float",
                Some(SemType::Boolean) => "boolean",
                Some(SemType::Nil) => "null",
                Some(SemType::Map) => "object",
                Some(SemType::Seq) => "array",
                None => "unknown",
            }
        }
    }
    .to_owned()
}

fn node_value_type(node: &TreeNode) -> String {
    match node_value_kind(node).as_str() {
        "int" | "float" => "number".to_owned(),
        other => other.to_owned(),
    }
}

fn node_preview_from_node(
    store: &TreeStore,
    node_id: NodeId,
    node: &TreeNode,
) -> DocumentNodePreview {
    let value_type = node_value_type(node);
    DocumentNodePreview {
        kind: super::snapshot::tree_kind_code(node.kind),
        sem_type: super::snapshot::sem_type_code(node.resolved_sem_type()),
        tag: node.tag.to_string_value(),
        value: store.value_string_for(node_id).unwrap_or_default(),
        value_type,
        is_scalar: matches!(node.kind, TreeNodeKind::Scalar),
    }
}

fn node_preview_for_path(
    document: &DecodedDocument,
    path_pattern: &str,
) -> Option<DocumentNodePreview> {
    let node_id = node_id_for_path(document, path_pattern, false)?;
    document
        .store
        .get(node_id)
        .map(|node| node_preview_from_node(&document.store, node_id, node))
}

fn source_slice_by_bytes(source: &str, start: u32, end: u32) -> String {
    let start = usize::try_from(start).ok();
    let end = usize::try_from(end).ok();
    match (start, end) {
        (Some(start), Some(end)) if start <= end && end <= source.len() => {
            source.get(start..end).unwrap_or_default().to_owned()
        }
        _ => String::new(),
    }
}

fn path_value_for_path(
    analysis: &AnalysisBundle,
    document: &DecodedDocument,
    path_pattern: &str,
) -> Option<DocumentPathValue> {
    let node_id = node_id_for_path(document, path_pattern, false)?;
    let node = document.store.get(node_id)?;
    let value = document.store.value_string_for(node_id).ok()?;
    let source_text = source_slice_by_bytes(&analysis.source, node.start_byte, node.end_byte);
    let display_text = if source_text.is_empty() {
        value.clone()
    } else {
        source_text.clone()
    };
    Some(DocumentPathValue {
        value_type: node_value_type(node),
        value,
        source_text,
        display_text,
    })
}

fn collect_field_labels(document: &DecodedDocument) -> Vec<String> {
    fn visit(store: &TreeStore, node_id: NodeId, labels: &mut BTreeSet<String>) {
        let Some(node) = store.get(node_id) else {
            return;
        };
        if node.kind == TreeNodeKind::Mapping {
            let mut index = 0usize;
            while index + 1 < node.content.len() {
                if let Some(key_node) = store.get(node.content[index]) {
                    if key_node.is_map_key {
                        let key = store.value_for(node.content[index]).unwrap_or_default();
                        if !key.is_empty() {
                            labels.insert(key.to_owned());
                        }
                    }
                }
                visit(store, node.content[index + 1], labels);
                index += 2;
            }
            return;
        }
        for child in &node.content {
            visit(store, *child, labels);
        }
    }

    let mut labels = BTreeSet::new();
    visit(&document.store, document.root, &mut labels);
    labels.into_iter().collect()
}

fn format_parsed_path(path: &[ParsedKey]) -> String {
    let segments = path
        .iter()
        .map(|segment| match segment {
            ParsedKey::Str(key) => path_seg_key(key),
            ParsedKey::Int(index) => path_seg_index(i32::try_from(*index).unwrap_or(0)),
        })
        .collect::<Vec<_>>();
    format_tree_path(&segments)
}

fn scalar_search_text(store: &TreeStore, node_id: NodeId) -> String {
    store
        .value_for(node_id)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn collect_search_items(document: &DecodedDocument) -> Vec<DocumentSearchItem> {
    fn push_value_item(
        store: &TreeStore,
        node_id: NodeId,
        path: &[ParsedKey],
        items: &mut Vec<DocumentSearchItem>,
    ) {
        let Some(_node) = store.get(node_id) else {
            return;
        };
        let value_text = scalar_search_text(store, node_id);
        if value_text.is_empty() {
            return;
        }
        let path_text = format_parsed_path(path);
        items.push(DocumentSearchItem {
            path: path_text.clone(),
            path_text,
            label: value_text.clone(),
            key_text: String::new(),
            value_text,
            target: QueryTargetKind::Value,
        });
    }

    fn visit(
        store: &TreeStore,
        node_id: NodeId,
        path: &mut Vec<ParsedKey>,
        items: &mut Vec<DocumentSearchItem>,
    ) {
        let Some(node) = store.get(node_id) else {
            return;
        };
        match node.kind {
            TreeNodeKind::Mapping => {
                let mut index = 0usize;
                while index + 1 < node.content.len() {
                    let key_id = node.content[index];
                    let value_id = node.content[index + 1];
                    let Some(_key_node) = store.get(key_id) else {
                        index += 2;
                        continue;
                    };
                    let key_text = scalar_search_text(store, key_id);
                    path.push(ParsedKey::Str(
                        store.value_string_for(key_id).unwrap_or_default(),
                    ));
                    if !key_text.is_empty() {
                        let value_text = store
                            .get(value_id)
                            .map(|_| scalar_search_text(store, value_id))
                            .unwrap_or_default();
                        let path_text = format_parsed_path(path);
                        items.push(DocumentSearchItem {
                            path: path_text.clone(),
                            path_text,
                            label: key_text.clone(),
                            key_text,
                            value_text,
                            target: QueryTargetKind::Key,
                        });
                    }
                    visit(store, value_id, path, items);
                    path.pop();
                    index += 2;
                }
            }
            TreeNodeKind::Sequence => {
                for (index, child_id) in node.content.iter().copied().enumerate() {
                    path.push(ParsedKey::Int(index as i64));
                    visit(store, child_id, path, items);
                    path.pop();
                }
            }
            TreeNodeKind::Scalar | TreeNodeKind::Alias | TreeNodeKind::Unknown => {
                push_value_item(store, node_id, path, items);
            }
        }
    }

    let mut items = Vec::new();
    visit(&document.store, document.root, &mut Vec::new(), &mut items);
    items
}
