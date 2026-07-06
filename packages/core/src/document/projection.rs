use crate::core::graph_builder::PathSeg as GraphPathSeg;
use crate::core::{NodeId, TreeNodeKind, TreeStore, graph_projection_service};

use super::protocol::{ProjectionDelta, ProjectionRequest, SnapshotReadResult};
use super::runtime::{DocumentRuntime, with_global_document_runtime};
use super::snapshot::DocumentSnapshot;

pub fn build_hover_subgraph_projection(
    request: &ProjectionRequest,
) -> Result<SnapshotReadResult<ProjectionDelta>, &'static str> {
    with_global_document_runtime(|runtime| {
        build_hover_subgraph_projection_in_runtime(runtime, request)
    })
    .ok_or("projection runtime error")?
}

fn build_hover_subgraph_projection_in_runtime(
    runtime: &DocumentRuntime,
    request: &ProjectionRequest,
) -> Result<SnapshotReadResult<ProjectionDelta>, &'static str> {
    let Some(snapshot) = runtime.snapshots.get(&request.snapshot_id.0) else {
        return Ok(SnapshotReadResult::SnapshotNotReady);
    };
    build_hover_subgraph_projection_for_snapshot(snapshot, &request.path)
        .map(|data| SnapshotReadResult::Ready { data })
}

fn build_hover_subgraph_projection_for_snapshot(
    snapshot: &DocumentSnapshot,
    path: &str,
) -> Result<ProjectionDelta, &'static str> {
    let analysis = snapshot.analysis.as_ref().ok_or("no analysis")?;
    let document = analysis.document.as_ref().ok_or("no document")?;
    let path = parse_snapshot_path(path).ok_or("invalid path")?;
    let root =
        resolve_node_for_path(&document.store, document.root, &path).unwrap_or(document.root);
    let root_path = snapshot_path_to_graph_path(&path);
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum SnapshotPathSeg {
    Key(String),
    Index(i32),
}

fn resolve_node_for_path(
    store: &TreeStore,
    root: NodeId,
    path: &[SnapshotPathSeg],
) -> Option<NodeId> {
    let mut current = root;
    for segment in path {
        let node = store.get(current)?;
        current = match segment {
            SnapshotPathSeg::Key(key) => {
                if node.kind != TreeNodeKind::Mapping {
                    return None;
                }
                node.content.chunks_exact(2).find_map(|pair| {
                    let key_node = store.get(pair[0])?;
                    (store.value_for(pair[0]).ok()? == key && key_node.is_map_key)
                        .then_some(pair[1])
                })?
            }
            SnapshotPathSeg::Index(index) => {
                if node.kind != TreeNodeKind::Sequence || *index < 0 {
                    return None;
                }
                *node.content.get(*index as usize)?
            }
        };
    }
    Some(current)
}

fn parse_snapshot_path(path: &str) -> Option<Vec<SnapshotPathSeg>> {
    if path.is_empty() || path == "$" {
        return Some(Vec::new());
    }
    let bytes = path.as_bytes();
    let mut index = usize::from(bytes.first() == Some(&b'$'));
    let mut segments = Vec::new();
    while index < bytes.len() {
        match bytes[index] {
            b'.' => {
                index += 1;
                let start = index;
                while index < bytes.len()
                    && matches!(bytes[index], b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'$')
                {
                    index += 1;
                }
                if start == index {
                    return None;
                }
                segments.push(SnapshotPathSeg::Key(path[start..index].to_owned()));
            }
            b'[' => {
                let end = path[index + 1..].find(']')? + index + 1;
                let inner = path[index + 1..end].trim();
                if inner.starts_with('"') {
                    segments.push(SnapshotPathSeg::Key(crate::core::unescape_json_string(
                        inner,
                    )?));
                } else {
                    segments.push(SnapshotPathSeg::Index(inner.parse::<i32>().ok()?));
                }
                index = end + 1;
            }
            _ => return None,
        }
    }
    Some(segments)
}

fn snapshot_path_to_graph_path(path: &[SnapshotPathSeg]) -> Vec<GraphPathSeg> {
    path.iter()
        .map(|segment| match segment {
            SnapshotPathSeg::Key(key) => GraphPathSeg::Key(key.clone()),
            SnapshotPathSeg::Index(index) => GraphPathSeg::Index((*index).max(0) as usize),
        })
        .collect()
}
