use super::*;
use crate::document::protocol::{
    DocumentFormattingSettings, DocumentParserSettings, DocumentTreeNode, GraphBoxArgs,
    GraphCellData, GraphDelta, GraphEdgeData, GraphPathSeg, GraphRowData, GraphTableData,
    GraphTextArgs,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct FinalArtifacts {
    tree: DocumentTreeNode,
    graph: CanonicalGraphView,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CanonicalGraphView {
    nodes: Vec<CanonicalGraphNode>,
    edges: Vec<CanonicalGraphEdge>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CanonicalGraphNode {
    kind: i32,
    path: Vec<GraphPathSeg>,
    depth: u32,
    box_args: GraphBoxArgs,
    meta: Option<CanonicalCell>,
    rows: Vec<CanonicalRow>,
    table: Option<CanonicalTable>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CanonicalGraphEdge {
    from_kind: i32,
    from_path: Vec<GraphPathSeg>,
    from_row: i32,
    to_kind: i32,
    to_path: Vec<GraphPathSeg>,
    to_row: i32,
    curve: [i32; 8],
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CanonicalTable {
    columns: Vec<CanonicalCell>,
    rows: Vec<CanonicalRow>,
    header_height: i32,
    total_height: i32,
    view_height: i32,
    row_height: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CanonicalRow {
    index: i32,
    box_args: GraphBoxArgs,
    cell_box_args: GraphBoxArgs,
    cells: Vec<CanonicalCell>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CanonicalCell {
    sem_type: u32,
    path: Vec<GraphPathSeg>,
    text: String,
    value: String,
    format_text: String,
    box_args: GraphBoxArgs,
    text_args: GraphTextArgs,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ArtifactSummary {
    tree: TreeSummary,
    graph: GraphSummary,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct TreeSummary {
    total_nodes: usize,
    max_depth: usize,
    kind_counts: BTreeMap<i32, usize>,
    sem_type_counts: BTreeMap<i32, usize>,
    top_level_entries: Vec<String>,
    fingerprint: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct GraphSummary {
    node_count: usize,
    edge_count: usize,
    node_kind_counts: BTreeMap<i32, usize>,
    max_depth: u32,
    table_node_count: usize,
    total_object_rows: usize,
    total_table_rows: usize,
    total_table_cells: usize,
    node_path_samples: Vec<String>,
    edge_samples: Vec<String>,
    table_samples: Vec<TableSummary>,
    semantic_fingerprint: String,
    layout_fingerprint: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct TableSummary {
    path: String,
    node_kind: i32,
    depth: u32,
    row_count: usize,
    column_headers: Vec<String>,
    first_row: Vec<String>,
    last_row: Vec<String>,
    total_height: i32,
    view_height: i32,
    row_height: i32,
    node_box: [i32; 4],
}

#[derive(Debug, Clone, Copy)]
struct StableHasher(u64);

impl Default for StableHasher {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl StableHasher {
    fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn write_bool(&mut self, value: bool) {
        self.write_bytes(&[u8::from(value)]);
    }

    fn write_u8(&mut self, value: u8) {
        self.write_bytes(&[value]);
    }

    fn write_i32(&mut self, value: i32) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }

    fn write_str(&mut self, value: &str) {
        self.write_bytes(value.as_bytes());
        self.write_u8(0xff);
    }

    fn finish_hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

fn load_fixture(relative_path: &str) -> String {
    read_repo_fixture(relative_path)
}

fn final_artifacts(document_key: &str, language: &str, chunks: &[&str]) -> FinalArtifacts {
    let (_snapshot_id, batches) = analyze_document_via_job(document_key, language, chunks);
    assert!(
        matches!(
            batches.last().and_then(|batch| batch.terminal.clone()),
            Some(JobTerminal::Completed)
        ),
        "{document_key} close should complete"
    );
    for (index, batch) in batches.iter().enumerate().skip(1).take(chunks.len()) {
        assert!(
            batch.terminal.is_none(),
            "{document_key} chunk batch {index} should stay open"
        );
    }

    let snapshot =
        stored_snapshot_for_document(document_key).expect("streaming snapshot should be stored");
    let payload = snapshot
        .analysis_payload(true)
        .expect("streaming snapshot should keep analysis payload");
    let tree = payload
        .tree
        .as_ref()
        .map(normalize_tree)
        .expect("streaming snapshot should keep final tree");
    // Reconstruct the full graph from the analysis document's TreeStore,
    // not from the snapshot's graph (which now stores only the close-phase
    // remaining delta — empty for fully-parsed documents).
    let graph = snapshot
        .analysis
        .as_ref()
        .and_then(|analysis| analysis.document.as_ref())
        .map(|decoded| {
            canonical_graph_view(
                &crate::graph::graph_projection_service::build_initial_projection_delta(
                    &decoded.store,
                    decoded.root,
                    language,
                    Some(document_key),
                ),
            )
        })
        .expect("streaming snapshot should keep decoded document for graph rebuild");

    FinalArtifacts { tree, graph }
}

fn normalize_tree(node: &DocumentTreeNode) -> DocumentTreeNode {
    let mut normalized = DocumentTreeNode {
        kind: node.kind,
        sem_type: node.sem_type,
        tag: node.tag.clone(),
        value: node.value.clone(),
        children: Vec::new(),
    };

    if node.kind == 1 {
        let mut index = 0usize;
        while index + 1 < node.children.len() {
            let key = &node.children[index];
            let value = &node.children[index + 1];
            normalized.children.push(normalize_tree(key));
            normalized.children.push(normalize_tree(value));
            index += 2;
        }
        return normalized;
    }

    normalized.children = node.children.iter().map(normalize_tree).collect();
    normalized
}

fn canonical_graph_view(delta: &GraphDelta) -> CanonicalGraphView {
    let mut nodes_by_handle = BTreeMap::<u32, crate::document::protocol::GraphNodeData>::new();
    for node in delta.nodes_added.iter().chain(delta.nodes_updated.iter()) {
        nodes_by_handle.insert(node.render_handle, node.clone());
    }
    for render_handle in &delta.nodes_removed {
        nodes_by_handle.remove(render_handle);
    }

    let mut edges_by_handle = BTreeMap::<(u32, u32), GraphEdgeData>::new();
    for edge in &delta.edges_added {
        edges_by_handle.insert(
            (edge.from_render_handle, edge.to_render_handle),
            edge.clone(),
        );
    }
    for edge in &delta.edges_removed {
        edges_by_handle.remove(&(edge.from, edge.to));
    }

    let mut row_index_maps = BTreeMap::<u32, BTreeMap<i32, i32>>::new();
    let mut nodes = Vec::new();
    for (render_handle, raw_node) in &nodes_by_handle {
        if let Some((node, row_index_map)) = canonical_node(raw_node) {
            row_index_maps.insert(*render_handle, row_index_map);
            nodes.push(node);
        }
    }
    nodes.sort_by(|left, right| canonical_node_sort_key(left).cmp(&canonical_node_sort_key(right)));

    let mut edges = edges_by_handle
        .values()
        .filter_map(|edge| canonical_edge(edge, &row_index_maps))
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| canonical_edge_sort_key(left).cmp(&canonical_edge_sort_key(right)));

    CanonicalGraphView { nodes, edges }
}

fn remap_row_index(
    row_index_maps: &BTreeMap<u32, BTreeMap<i32, i32>>,
    render_handle: u32,
    row_index: i32,
) -> i32 {
    row_index_maps
        .get(&render_handle)
        .and_then(|map| map.get(&row_index).copied())
        .unwrap_or(row_index)
}

fn canonical_node(
    node: &crate::document::protocol::GraphNodeData,
) -> Option<(CanonicalGraphNode, BTreeMap<i32, i32>)> {
    let meta = node.meta.as_ref().and_then(canonical_cell);
    let (rows, row_index_map) = canonical_rows(&node.rows);
    let (table, table_row_index_map) = match node.table.as_ref() {
        Some(table) => {
            let (table, row_index_map) = canonical_table(table);
            (Some(table), row_index_map)
        }
        None => (None, BTreeMap::new()),
    };
    let effective_row_index_map = if table.is_some() {
        table_row_index_map
    } else {
        row_index_map
    };

    Some((
        CanonicalGraphNode {
            kind: node.kind,
            path: node.path.clone(),
            depth: node.depth,
            box_args: node.box_args,
            meta,
            rows,
            table,
        },
        effective_row_index_map,
    ))
}

fn canonical_edge(
    edge: &GraphEdgeData,
    row_index_maps: &BTreeMap<u32, BTreeMap<i32, i32>>,
) -> Option<CanonicalGraphEdge> {
    Some(CanonicalGraphEdge {
        from_kind: edge.from_kind,
        from_path: edge.from_path.clone(),
        from_row: remap_row_index(row_index_maps, edge.from_render_handle, edge.from_row),
        to_kind: edge.to_kind,
        to_path: edge.to_path.clone(),
        to_row: remap_row_index(row_index_maps, edge.to_render_handle, edge.to_row),
        curve: [
            edge.bezier_from_x,
            edge.bezier_from_y,
            edge.bezier_c1x,
            edge.bezier_c1y,
            edge.bezier_c2x,
            edge.bezier_c2y,
            edge.bezier_to_x,
            edge.bezier_to_y,
        ],
    })
}

fn canonical_table(table: &GraphTableData) -> (CanonicalTable, BTreeMap<i32, i32>) {
    let (rows, row_index_map) = canonical_rows(&table.rows);
    (
        CanonicalTable {
            columns: table.columns.iter().filter_map(canonical_cell).collect(),
            rows,
            header_height: table.header_height,
            total_height: table.total_height,
            view_height: table.view_height,
            row_height: table.row_height,
        },
        row_index_map,
    )
}

fn canonical_rows(rows: &[GraphRowData]) -> (Vec<CanonicalRow>, BTreeMap<i32, i32>) {
    let mut next_index = 0i32;
    let mut row_index_map = BTreeMap::new();
    let mut canonical_rows = Vec::new();
    for row in rows {
        if let Some(mut canonical_row) = canonical_row(row) {
            row_index_map.insert(row.index, next_index);
            canonical_row.index = next_index;
            canonical_rows.push(canonical_row);
            next_index += 1;
        }
    }
    (canonical_rows, row_index_map)
}

fn canonical_row(row: &GraphRowData) -> Option<CanonicalRow> {
    let cells = row
        .cells
        .iter()
        .filter_map(canonical_cell)
        .collect::<Vec<_>>();
    if cells.is_empty() {
        return None;
    }

    Some(CanonicalRow {
        index: row.index,
        box_args: row.box_args,
        cell_box_args: row.cell_box_args,
        cells,
    })
}

fn canonical_cell(cell: &GraphCellData) -> Option<CanonicalCell> {
    Some(CanonicalCell {
        sem_type: cell.sem_type,
        path: cell.path.clone(),
        text: cell.text.clone(),
        value: cell.value.clone(),
        format_text: cell.format_text.clone(),
        box_args: cell.box_args,
        text_args: cell.text_args.clone(),
    })
}

fn canonical_node_sort_key(node: &CanonicalGraphNode) -> (String, i32, u32) {
    (graph_path_to_string(&node.path), node.kind, node.depth)
}

fn canonical_edge_sort_key(edge: &CanonicalGraphEdge) -> (String, i32, String, i32) {
    (
        graph_path_to_string(&edge.from_path),
        edge.from_row,
        graph_path_to_string(&edge.to_path),
        edge.to_row,
    )
}

fn split_inside_needle(source: &str, needle: &str) -> usize {
    let start = split_before_needle(source, needle);
    let split = start + midpoint_char_boundary(needle);
    assert!(
        split > 0 && split < source.len() && source.is_char_boundary(split),
        "needle split must land strictly inside the source on a char boundary"
    );
    split
}

fn split_at_or_after_char_boundary(source: &str, approx: usize) -> usize {
    let upper = source.len().saturating_sub(1);
    let mut forward = approx.min(upper);
    while forward < source.len() && !source.is_char_boundary(forward) {
        forward += 1;
    }
    if forward > 0 && forward < source.len() && source.is_char_boundary(forward) {
        return forward;
    }

    let mut backward = approx.min(upper);
    while backward > 0 && !source.is_char_boundary(backward) {
        backward -= 1;
    }
    assert!(
        backward > 0 && backward < source.len() && source.is_char_boundary(backward),
        "approximate split should resolve to an interior char boundary"
    );
    backward
}

fn chunk_slices<'a>(source: &'a str, split_points: &[usize]) -> Vec<&'a str> {
    let mut points = split_points.to_vec();
    points.sort_unstable();
    points.dedup();
    assert!(
        points.len() == split_points.len(),
        "split points must be unique: {split_points:?}"
    );

    let mut chunks = Vec::with_capacity(points.len() + 1);
    let mut start = 0usize;
    for split in points {
        assert!(
            split > start && split < source.len() && source.is_char_boundary(split),
            "split points must be increasing interior char boundaries"
        );
        chunks.push(&source[start..split]);
        start = split;
    }
    chunks.push(&source[start..]);
    chunks
}

fn assert_matches_baseline(
    case_label: &str,
    document_key: &str,
    language: &str,
    source: &str,
    split_points: &[usize],
    baseline: &FinalArtifacts,
) {
    let chunks = chunk_slices(source, split_points);
    let candidate = final_artifacts(document_key, language, &chunks);
    if &candidate != baseline {
        let candidate_summary = summarize_artifacts(&candidate);
        let baseline_summary = summarize_artifacts(baseline);
        panic!(
            "{case_label} should match single-chunk baseline\n\ncandidate summary:\n{candidate_summary:#?}\n\nbaseline summary:\n{baseline_summary:#?}"
        );
    }
}

fn summarize_artifacts(artifacts: &FinalArtifacts) -> ArtifactSummary {
    ArtifactSummary {
        tree: summarize_tree(&artifacts.tree),
        graph: summarize_graph(&artifacts.graph),
    }
}

fn summarize_tree(root: &DocumentTreeNode) -> TreeSummary {
    fn walk(
        node: &DocumentTreeNode,
        depth: usize,
        kind_counts: &mut BTreeMap<i32, usize>,
        sem_type_counts: &mut BTreeMap<i32, usize>,
        total_nodes: &mut usize,
        max_depth: &mut usize,
        hasher: &mut StableHasher,
    ) {
        *total_nodes += 1;
        *max_depth = (*max_depth).max(depth);
        *kind_counts.entry(node.kind).or_default() += 1;
        *sem_type_counts.entry(node.sem_type).or_default() += 1;
        hasher.write_i32(node.kind);
        hasher.write_i32(node.sem_type);
        hasher.write_str(&node.tag);
        hasher.write_str(&node.value);
        hasher.write_usize(node.children.len());
        for child in &node.children {
            walk(
                child,
                depth + 1,
                kind_counts,
                sem_type_counts,
                total_nodes,
                max_depth,
                hasher,
            );
        }
    }

    let mut kind_counts = BTreeMap::new();
    let mut sem_type_counts = BTreeMap::new();
    let mut total_nodes = 0usize;
    let mut max_depth = 0usize;
    let mut hasher = StableHasher::default();
    walk(
        root,
        0,
        &mut kind_counts,
        &mut sem_type_counts,
        &mut total_nodes,
        &mut max_depth,
        &mut hasher,
    );

    let mut top_level_entries = Vec::new();
    if root.kind == 1 {
        let mut index = 0usize;
        while index < root.children.len() {
            let key = &root.children[index];
            top_level_entries.push(if key.value.is_empty() {
                format!("#{index}")
            } else {
                key.value.clone()
            });
            index += 2;
        }
    } else {
        top_level_entries.extend(
            root.children
                .iter()
                .take(12)
                .map(|child| child.value.clone()),
        );
    }

    TreeSummary {
        total_nodes,
        max_depth,
        kind_counts,
        sem_type_counts,
        top_level_entries,
        fingerprint: hasher.finish_hex(),
    }
}

fn summarize_graph(graph: &CanonicalGraphView) -> GraphSummary {
    let mut node_kind_counts = BTreeMap::new();
    let mut max_depth = 0u32;
    let mut table_node_count = 0usize;
    let mut total_object_rows = 0usize;
    let mut total_table_rows = 0usize;
    let mut total_table_cells = 0usize;
    let mut node_path_samples = Vec::new();
    let mut edge_samples = Vec::new();
    let mut table_samples = Vec::new();
    let mut semantic_hasher = StableHasher::default();
    let mut layout_hasher = StableHasher::default();

    for node in &graph.nodes {
        *node_kind_counts.entry(node.kind).or_default() += 1;
        max_depth = max_depth.max(node.depth);
        total_object_rows += node.rows.len();
        if node_path_samples.len() < 8 {
            node_path_samples.push(format!(
                "k{}|d{}|{}",
                node.kind,
                node.depth,
                graph_path_to_string(&node.path)
            ));
        }
        hash_node_semantics(&mut semantic_hasher, node);
        hash_node_layout(&mut layout_hasher, node);
        if let Some(table) = node.table.as_ref() {
            table_node_count += 1;
            total_table_rows += table.rows.len();
            total_table_cells += table.columns.len();
            total_table_cells += table.rows.iter().map(|row| row.cells.len()).sum::<usize>();
            if table_samples.len() < 4 {
                table_samples.push(TableSummary {
                    path: graph_path_to_string(&node.path),
                    node_kind: node.kind,
                    depth: node.depth,
                    row_count: table.rows.len(),
                    column_headers: table.columns.iter().map(|cell| cell.text.clone()).collect(),
                    first_row: table.rows.first().map(row_values).unwrap_or_default(),
                    last_row: table.rows.last().map(row_values).unwrap_or_default(),
                    total_height: table.total_height,
                    view_height: table.view_height,
                    row_height: table.row_height,
                    node_box: [
                        node.box_args.x,
                        node.box_args.y,
                        node.box_args.width,
                        node.box_args.height,
                    ],
                });
            }
        }
    }

    for edge in &graph.edges {
        if edge_samples.len() < 8 {
            edge_samples.push(format!(
                "{}:{} -> {}:{}",
                graph_path_to_string(&edge.from_path),
                edge.from_row,
                graph_path_to_string(&edge.to_path),
                edge.to_row,
            ));
        }
        hash_edge_semantics(&mut semantic_hasher, edge);
        hash_edge_layout(&mut layout_hasher, edge);
    }

    GraphSummary {
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        node_kind_counts,
        max_depth,
        table_node_count,
        total_object_rows,
        total_table_rows,
        total_table_cells,
        node_path_samples,
        edge_samples,
        table_samples,
        semantic_fingerprint: semantic_hasher.finish_hex(),
        layout_fingerprint: layout_hasher.finish_hex(),
    }
}

fn graph_path_to_string(path: &[GraphPathSeg]) -> String {
    if path.is_empty() {
        return "$".to_owned();
    }
    let mut out = String::from("$");
    for segment in path {
        if !segment.key.is_empty() {
            out.push('.');
            out.push_str(&segment.key);
        } else {
            out.push('[');
            out.push_str(&segment.index.to_string());
            out.push(']');
        }
    }
    out
}

fn row_values(row: &CanonicalRow) -> Vec<String> {
    row.cells.iter().map(|cell| cell.value.clone()).collect()
}

fn hash_node_semantics(hasher: &mut StableHasher, node: &CanonicalGraphNode) {
    hasher.write_i32(node.kind);
    hasher.write_u32(node.depth);
    write_graph_path(hasher, &node.path);
    write_optional_cell_semantics(hasher, node.meta.as_ref());
    hasher.write_usize(node.rows.len());
    for row in &node.rows {
        write_row_semantics(hasher, row);
    }
    match node.table.as_ref() {
        Some(table) => {
            hasher.write_bool(true);
            hasher.write_usize(table.columns.len());
            for column in &table.columns {
                write_cell_semantics(hasher, column);
            }
            hasher.write_usize(table.rows.len());
            for row in &table.rows {
                write_row_semantics(hasher, row);
            }
        }
        None => hasher.write_bool(false),
    }
}

fn hash_node_layout(hasher: &mut StableHasher, node: &CanonicalGraphNode) {
    write_box(hasher, &node.box_args);
    write_optional_cell_layout(hasher, node.meta.as_ref());
    hasher.write_usize(node.rows.len());
    for row in &node.rows {
        write_row_layout(hasher, row);
    }
    match node.table.as_ref() {
        Some(table) => {
            hasher.write_bool(true);
            hasher.write_i32(table.header_height);
            hasher.write_i32(table.total_height);
            hasher.write_i32(table.view_height);
            hasher.write_i32(table.row_height);
            hasher.write_usize(table.columns.len());
            for column in &table.columns {
                write_cell_layout(hasher, column);
            }
            hasher.write_usize(table.rows.len());
            for row in &table.rows {
                write_row_layout(hasher, row);
            }
        }
        None => hasher.write_bool(false),
    }
}

fn hash_edge_semantics(hasher: &mut StableHasher, edge: &CanonicalGraphEdge) {
    hasher.write_i32(edge.from_kind);
    write_graph_path(hasher, &edge.from_path);
    hasher.write_i32(edge.from_row);
    hasher.write_i32(edge.to_kind);
    write_graph_path(hasher, &edge.to_path);
    hasher.write_i32(edge.to_row);
}

fn hash_edge_layout(hasher: &mut StableHasher, edge: &CanonicalGraphEdge) {
    for value in edge.curve {
        hasher.write_i32(value);
    }
}

fn write_graph_path(hasher: &mut StableHasher, path: &[GraphPathSeg]) {
    hasher.write_usize(path.len());
    for segment in path {
        hasher.write_i32(segment.tag);
        hasher.write_str(&segment.key);
        hasher.write_i32(segment.index);
    }
}

fn write_optional_cell_semantics(hasher: &mut StableHasher, cell: Option<&CanonicalCell>) {
    match cell {
        Some(cell) => {
            hasher.write_bool(true);
            write_cell_semantics(hasher, cell);
        }
        None => hasher.write_bool(false),
    }
}

fn write_optional_cell_layout(hasher: &mut StableHasher, cell: Option<&CanonicalCell>) {
    match cell {
        Some(cell) => {
            hasher.write_bool(true);
            write_cell_layout(hasher, cell);
        }
        None => hasher.write_bool(false),
    }
}

fn write_row_semantics(hasher: &mut StableHasher, row: &CanonicalRow) {
    hasher.write_i32(row.index);
    hasher.write_usize(row.cells.len());
    for cell in &row.cells {
        write_cell_semantics(hasher, cell);
    }
}

fn write_row_layout(hasher: &mut StableHasher, row: &CanonicalRow) {
    hasher.write_i32(row.index);
    write_box(hasher, &row.box_args);
    write_box(hasher, &row.cell_box_args);
    hasher.write_usize(row.cells.len());
    for cell in &row.cells {
        write_cell_layout(hasher, cell);
    }
}

fn write_cell_semantics(hasher: &mut StableHasher, cell: &CanonicalCell) {
    hasher.write_u32(cell.sem_type);
    write_graph_path(hasher, &cell.path);
    hasher.write_str(&cell.text);
    hasher.write_str(&cell.value);
    hasher.write_str(&cell.format_text);
}

fn write_cell_layout(hasher: &mut StableHasher, cell: &CanonicalCell) {
    write_box(hasher, &cell.box_args);
    write_text_args_layout(hasher, &cell.text_args);
}

fn write_box(hasher: &mut StableHasher, box_args: &GraphBoxArgs) {
    hasher.write_i32(box_args.x);
    hasher.write_i32(box_args.y);
    hasher.write_i32(box_args.width);
    hasher.write_i32(box_args.height);
    hasher.write_i32(box_args.corner_radius);
}

fn write_text_args_layout(hasher: &mut StableHasher, text_args: &GraphTextArgs) {
    hasher.write_i32(text_args.x);
    hasher.write_i32(text_args.y);
    hasher.write_i32(text_args.width);
    hasher.write_i32(text_args.height);
    hasher.write_u8(text_args.text_align);
    hasher.write_u8(text_args.text_vertical_align);
    hasher.write_bool(text_args.editable);
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DemoTreePatch {
    DocumentStarted {
        root: u32,
    },
    NodeInserted {
        node_id: u32,
        parent: Option<u32>,
        sequence_index: Option<u32>,
        kind: i32,
        sem_type: i32,
        tag: String,
        value: String,
    },
    NodeSealed {
        node_id: u32,
    },
    DocumentEnded {
        root: u32,
    },
}

#[derive(Debug)]
struct DemoTreeNode {
    kind: i32,
    sem_type: i32,
    tag: String,
    value: String,
    children: Vec<u32>,
    sealed: bool,
}

#[derive(Debug, Default)]
struct DemoTreeStore {
    root: Option<u32>,
    nodes: Vec<DemoTreeNode>,
    ended: bool,
}

impl DemoTreeStore {
    fn apply(&mut self, patch: &DemoTreePatch) {
        assert!(
            !self.ended,
            "tree patch after DocumentEnded must be rejected"
        );
        match patch {
            DemoTreePatch::DocumentStarted { root } => {
                assert!(self.root.is_none(), "DocumentStarted must be emitted once");
                assert!(
                    self.nodes.is_empty(),
                    "DocumentStarted must be the first structure patch"
                );
                self.root = Some(*root);
            }
            DemoTreePatch::NodeInserted {
                node_id,
                parent,
                sequence_index,
                kind,
                sem_type,
                tag,
                value,
            } => {
                assert_eq!(
                    *node_id as usize,
                    self.nodes.len(),
                    "NodeId must be append-only and monotonic"
                );
                match parent {
                    Some(parent_id) => {
                        let parent_node = self
                            .nodes
                            .get_mut(*parent_id as usize)
                            .expect("parent must exist before child");
                        assert!(!parent_node.sealed, "cannot append child to sealed parent");
                        assert_eq!(
                            Some(parent_node.children.len() as u32),
                            *sequence_index,
                            "sequence_index must match parent append position"
                        );
                        parent_node.children.push(*node_id);
                    }
                    None => {
                        assert_eq!(
                            self.root,
                            Some(*node_id),
                            "root id must match DocumentStarted"
                        );
                        assert!(
                            sequence_index.is_none(),
                            "root must not carry a sequence_index"
                        );
                    }
                }
                self.nodes.push(DemoTreeNode {
                    kind: *kind,
                    sem_type: *sem_type,
                    tag: tag.clone(),
                    value: value.clone(),
                    children: Vec::new(),
                    sealed: false,
                });
            }
            DemoTreePatch::NodeSealed { node_id } => {
                let node = self
                    .nodes
                    .get_mut(*node_id as usize)
                    .expect("sealed node must exist");
                assert!(!node.sealed, "node must be sealed exactly once");
                node.sealed = true;
            }
            DemoTreePatch::DocumentEnded { root } => {
                assert_eq!(self.root, Some(*root), "DocumentEnded root must match");
                let root_node = self.nodes.get(*root as usize).expect("root must exist");
                assert!(root_node.sealed, "root must be sealed before DocumentEnded");
                assert!(
                    self.nodes.iter().all(|node| node.sealed),
                    "all nodes must be sealed before DocumentEnded"
                );
                self.ended = true;
            }
        }
    }

    fn materialize(&self) -> DocumentTreeNode {
        assert!(self.ended, "tree can only materialize after DocumentEnded");
        let root = self.root.expect("root must be set");
        self.materialize_node(root)
    }

    fn materialize_node(&self, node_id: u32) -> DocumentTreeNode {
        let node = &self.nodes[node_id as usize];
        DocumentTreeNode {
            kind: node.kind,
            sem_type: node.sem_type,
            tag: node.tag.clone(),
            value: node.value.clone(),
            children: node
                .children
                .iter()
                .map(|child_id| self.materialize_node(*child_id))
                .collect(),
        }
    }
}

fn emit_demo_tree_patches(root: &DocumentTreeNode) -> Vec<DemoTreePatch> {
    fn emit_node(
        node: &DocumentTreeNode,
        parent: Option<u32>,
        sequence_index: Option<u32>,
        next_node_id: &mut u32,
        patches: &mut Vec<DemoTreePatch>,
    ) -> u32 {
        let node_id = *next_node_id;
        *next_node_id += 1;

        if parent.is_none() {
            patches.push(DemoTreePatch::DocumentStarted { root: node_id });
        }

        patches.push(DemoTreePatch::NodeInserted {
            node_id,
            parent,
            sequence_index,
            kind: node.kind,
            sem_type: node.sem_type,
            tag: node.tag.clone(),
            value: node.value.clone(),
        });

        for (index, child) in node.children.iter().enumerate() {
            emit_node(
                child,
                Some(node_id),
                Some(index as u32),
                next_node_id,
                patches,
            );
        }

        patches.push(DemoTreePatch::NodeSealed { node_id });

        if parent.is_none() {
            patches.push(DemoTreePatch::DocumentEnded { root: node_id });
        }

        node_id
    }

    let mut patches = Vec::new();
    let mut next_node_id = 0;
    emit_node(root, None, None, &mut next_node_id, &mut patches);
    patches
}

fn apply_demo_tree_patches(patches: &[DemoTreePatch]) -> DocumentTreeNode {
    let mut store = DemoTreeStore::default();
    for patch in patches {
        store.apply(patch);
    }
    store.materialize()
}

#[derive(Debug, Clone)]
enum DemoTablePatch {
    TableCreated {
        node_handle: usize,
        columns: Vec<CanonicalCell>,
        header_height: i32,
        total_height: i32,
        view_height: i32,
        row_height: i32,
    },
    RowsAppended {
        node_handle: usize,
        start_index: usize,
        rows: Vec<CanonicalRow>,
    },
}

#[derive(Debug, Clone, Default)]
struct DemoGraphPatch {
    nodes_added: Vec<(usize, CanonicalGraphNode)>,
    nodes_updated: Vec<(usize, CanonicalGraphNode)>,
    nodes_removed: Vec<usize>,
    edges_added: Vec<CanonicalGraphEdge>,
    edges_removed: Vec<usize>,
    table_patches: Vec<DemoTablePatch>,
}

#[derive(Debug, Clone)]
struct DemoProjectionPatch {
    clear: bool,
    graph_data: Option<DemoGraphPatch>,
    patch_seq: u64,
    base_graph_version: u64,
    graph_version: u64,
}

#[derive(Debug, Default)]
struct DemoGraphRenderState {
    version: u64,
    nodes_by_handle: BTreeMap<usize, CanonicalGraphNode>,
    node_order: Vec<usize>,
    edges: Vec<CanonicalGraphEdge>,
}

impl DemoGraphRenderState {
    fn apply(&mut self, patch: &DemoProjectionPatch) -> Result<(), String> {
        if patch.base_graph_version != self.version {
            return Err(format!(
                "base graph version mismatch: state={}, patch={}",
                self.version, patch.base_graph_version
            ));
        }

        if patch.clear {
            self.nodes_by_handle.clear();
            self.node_order.clear();
            self.edges.clear();
        }

        if let Some(graph) = patch.graph_data.as_ref() {
            for handle in &graph.nodes_removed {
                self.nodes_by_handle.remove(handle);
                self.node_order.retain(|candidate| candidate != handle);
            }

            for (handle, node) in &graph.nodes_added {
                if self.nodes_by_handle.insert(*handle, node.clone()).is_some() {
                    return Err(format!("node handle {handle} added twice"));
                }
                self.node_order.push(*handle);
            }

            for (handle, node) in &graph.nodes_updated {
                if !self.nodes_by_handle.contains_key(handle) {
                    return Err(format!("node handle {handle} updated before add"));
                }
                self.nodes_by_handle.insert(*handle, node.clone());
            }

            for edge_index in graph.edges_removed.iter().rev() {
                if *edge_index >= self.edges.len() {
                    return Err(format!("edge index {edge_index} removed out of range"));
                }
                self.edges.remove(*edge_index);
            }

            self.edges.extend(graph.edges_added.iter().cloned());

            for table_patch in &graph.table_patches {
                self.apply_table_patch(table_patch)?;
            }
        }

        self.version = patch.graph_version;
        Ok(())
    }

    fn apply_table_patch(&mut self, patch: &DemoTablePatch) -> Result<(), String> {
        match patch {
            DemoTablePatch::TableCreated {
                node_handle,
                columns,
                header_height,
                total_height,
                view_height,
                row_height,
            } => {
                let node = self
                    .nodes_by_handle
                    .get_mut(node_handle)
                    .ok_or_else(|| format!("table node {node_handle} must exist"))?;
                let table = node.table.get_or_insert_with(|| CanonicalTable {
                    columns: Vec::new(),
                    rows: Vec::new(),
                    header_height: 0,
                    total_height: 0,
                    view_height: 0,
                    row_height: 0,
                });
                if !table.rows.is_empty() {
                    return Err(format!(
                        "table {node_handle} created after rows were appended"
                    ));
                }
                table.columns = columns.clone();
                table.header_height = *header_height;
                table.total_height = *total_height;
                table.view_height = *view_height;
                table.row_height = *row_height;
            }
            DemoTablePatch::RowsAppended {
                node_handle,
                start_index,
                rows,
            } => {
                let node = self
                    .nodes_by_handle
                    .get_mut(node_handle)
                    .ok_or_else(|| format!("table node {node_handle} must exist"))?;
                let table = node
                    .table
                    .as_mut()
                    .ok_or_else(|| format!("table {node_handle} must be created"))?;
                if table.rows.len() != *start_index {
                    return Err(format!(
                        "table {node_handle} row append gap: len={}, start={start_index}",
                        table.rows.len()
                    ));
                }
                table.rows.extend(rows.iter().cloned());
            }
        }
        Ok(())
    }

    fn materialize(&self) -> CanonicalGraphView {
        CanonicalGraphView {
            nodes: self
                .node_order
                .iter()
                .map(|handle| {
                    self.nodes_by_handle
                        .get(handle)
                        .expect("node order must reference an existing node")
                        .clone()
                })
                .collect(),
            edges: self.edges.clone(),
        }
    }
}

fn emit_demo_graph_patches(graph: &CanonicalGraphView) -> Vec<DemoProjectionPatch> {
    fn push_patch(
        patches: &mut Vec<DemoProjectionPatch>,
        clear: bool,
        graph_data: Option<DemoGraphPatch>,
    ) {
        let base_graph_version = patches.last().map_or(0, |patch| patch.graph_version);
        let graph_version = base_graph_version + 1;
        patches.push(DemoProjectionPatch {
            clear,
            graph_data,
            patch_seq: patches.len() as u64 + 1,
            base_graph_version,
            graph_version,
        });
    }

    let mut patches = Vec::new();
    push_patch(&mut patches, true, None);

    for (handle, node) in graph.nodes.iter().enumerate() {
        let mut initial_node = node.clone();
        let table = initial_node.table.take();
        if let Some(table) = table.as_ref() {
            initial_node.table = Some(CanonicalTable {
                columns: Vec::new(),
                rows: Vec::new(),
                header_height: table.header_height,
                total_height: table.total_height,
                view_height: table.view_height,
                row_height: table.row_height,
            });
        }

        let mut patch = DemoGraphPatch::default();
        patch.nodes_added.push((handle, initial_node));
        if let Some(table) = table.as_ref() {
            patch.table_patches.push(DemoTablePatch::TableCreated {
                node_handle: handle,
                columns: table.columns.clone(),
                header_height: table.header_height,
                total_height: table.total_height,
                view_height: table.view_height,
                row_height: table.row_height,
            });
        }
        push_patch(&mut patches, false, Some(patch));

        if let Some(table) = table.as_ref() {
            for (index, row) in table.rows.iter().enumerate() {
                let mut patch = DemoGraphPatch::default();
                patch.table_patches.push(DemoTablePatch::RowsAppended {
                    node_handle: handle,
                    start_index: index,
                    rows: vec![row.clone()],
                });
                push_patch(&mut patches, false, Some(patch));
            }
        }
    }

    for edge in &graph.edges {
        let mut patch = DemoGraphPatch::default();
        patch.edges_added.push(edge.clone());
        push_patch(&mut patches, false, Some(patch));
    }

    patches
}

fn apply_demo_graph_patches(patches: &[DemoProjectionPatch]) -> CanonicalGraphView {
    let mut state = DemoGraphRenderState::default();
    for patch in patches {
        state.apply(patch).expect("demo graph patch should apply");
    }
    state.materialize()
}

fn assert_demo_graph_patches_are_incremental(
    patches: &[DemoProjectionPatch],
    graph: &CanonicalGraphView,
) {
    assert!(!patches.is_empty(), "graph patch stream must not be empty");

    let mut expected_version = 0;
    let mut nodes_added = 0usize;
    let mut edges_added = 0usize;
    let mut rows_appended = 0usize;

    for (index, patch) in patches.iter().enumerate() {
        assert_eq!(
            patch.patch_seq,
            index as u64 + 1,
            "patch_seq must be monotonic"
        );
        assert_eq!(
            patch.base_graph_version, expected_version,
            "base_graph_version must chain"
        );
        expected_version += 1;
        assert_eq!(
            patch.graph_version, expected_version,
            "graph_version must advance by one"
        );

        if let Some(graph_patch) = patch.graph_data.as_ref() {
            assert!(
                graph_patch.nodes_updated.is_empty(),
                "table row appends must not produce node updates"
            );
            nodes_added += graph_patch.nodes_added.len();
            edges_added += graph_patch.edges_added.len();
            for table_patch in &graph_patch.table_patches {
                if let DemoTablePatch::RowsAppended { rows, .. } = table_patch {
                    rows_appended += rows.len();
                }
            }
        }
    }

    let expected_rows_appended: usize = graph
        .nodes
        .iter()
        .filter_map(|node| node.table.as_ref())
        .map(|table| table.rows.len())
        .sum();
    assert_eq!(nodes_added, graph.nodes.len());
    assert_eq!(edges_added, graph.edges.len());
    assert_eq!(rows_appended, expected_rows_appended);
    assert!(
        rows_appended > 0,
        "fixture must exercise RowsAppended table patches"
    );
}

fn assert_demo_graph_version_mismatch_is_rejected(patches: &[DemoProjectionPatch]) {
    assert!(
        patches.len() > 2,
        "fixture must have enough graph patches to skip one"
    );
    let mut state = DemoGraphRenderState::default();
    state
        .apply(&patches[0])
        .expect("first demo graph patch should apply");
    assert!(
        state.apply(&patches[2]).is_err(),
        "skipping a patch must be rejected by base_graph_version"
    );
}

#[test]
fn wasm_document_patch_demo_materializes_simple_json_snapshot() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = load_fixture("example/simple.json");
    let expected = final_artifacts("simple-json-patch-demo", "json", &[&source]);

    let tree_patches = emit_demo_tree_patches(&expected.tree);
    let tree = apply_demo_tree_patches(&tree_patches);
    assert_eq!(tree, expected.tree);

    let graph_patches = emit_demo_graph_patches(&expected.graph);
    assert_demo_graph_patches_are_incremental(&graph_patches, &expected.graph);
    assert_demo_graph_version_mismatch_is_rejected(&graph_patches);
    let graph = apply_demo_graph_patches(&graph_patches);
    assert_eq!(graph, expected.graph);

    let baseline = FinalArtifacts { tree, graph };
    insta::assert_yaml_snapshot!("streaming_simple_json_final_artifacts", baseline);
}

#[test]
fn wasm_document_json_streaming_simple_fixture_snapshot() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = load_fixture("example/simple.json");
    let baseline = final_artifacts("simple-json-snapshot", "json", &[&source]);

    insta::assert_yaml_snapshot!("streaming_simple_json_final_artifacts", baseline);
}

#[test]
fn wasm_document_json_streaming_simple_fixture_split_variants_match_baseline() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = load_fixture("example/simple.json");
    let baseline = final_artifacts("simple-json-baseline", "json", &[&source]);
    let cases = [
        (
            "json table_with_header split",
            "simple-json-table-with-header",
            vec![split_before_needle(&source, r#"    {"h1": 21"#)],
        ),
        (
            "json table_without_header split",
            "simple-json-table-without-header",
            vec![split_before_needle(&source, r#""c"],"#)],
        ),
        (
            "json unicode split",
            "simple-json-unicode",
            vec![split_inside_needle(&source, "你好")],
        ),
        (
            "json jwt split",
            "simple-json-jwt",
            vec![split_inside_needle(
                &source,
                "eyJzdWIiOiJ0cmVlYXNlIiwicm9sZSI6ImRlbW8i",
            )],
        ),
        (
            "json preview uri split",
            "simple-json-preview-uri",
            vec![split_before_needle(
                &source,
                "https://treease.com/path?redirect=",
            )],
        ),
    ];

    for (label, document_key, split_points) in cases {
        assert_matches_baseline(
            label,
            document_key,
            "json",
            &source,
            &split_points,
            &baseline,
        );
    }
}

#[test]
fn wasm_document_json_streaming_complex_fixture_summary_snapshot() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = load_fixture("test/fixtures/json/complex.1.json");
    let baseline = final_artifacts("complex-json-snapshot", "json", &[&source]);

    insta::assert_yaml_snapshot!(
        "streaming_complex_json_final_artifact_summary",
        summarize_artifacts(&baseline)
    );
}

#[test]
fn wasm_document_json_streaming_complex_fixture_split_variants_match_baseline() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = load_fixture("test/fixtures/json/complex.1.json");
    let baseline = final_artifacts("complex-json-baseline", "json", &[&source]);
    let featureful_split = split_inside_escape_or_json_string(&source);
    let midpoint_split = midpoint_char_boundary(&source);

    assert_matches_baseline(
        "complex json featureful split",
        "complex-json-featureful",
        "json",
        &source,
        &[featureful_split, midpoint_split],
        &baseline,
    );
}

#[test]
fn wasm_document_json_streaming_1mb_min_fixture_summary_snapshot() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = load_fixture("test/fixtures/json/1MB-min.1.json");
    let baseline = final_artifacts("min-1mb-json-snapshot", "json", &[&source]);

    insta::assert_yaml_snapshot!(
        "streaming_1mb_min_json_final_artifact_summary",
        summarize_artifacts(&baseline)
    );
}

#[test]
fn wasm_document_json_streaming_1mb_min_fixture_split_variants_match_baseline() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = load_fixture("test/fixtures/json/1MB-min.1.json");
    let baseline = final_artifacts("min-1mb-json-baseline", "json", &[&source]);
    let first_third = split_at_or_after_char_boundary(&source, source.len() / 3);
    let second_third = split_at_or_after_char_boundary(&source, source.len() * 2 / 3);

    assert_matches_baseline(
        "1MB min json thirds split",
        "min-1mb-json-thirds",
        "json",
        &source,
        &[first_third, second_third],
        &baseline,
    );
}

#[derive(Debug, Clone)]
enum RealEventTreePatch {
    DocumentStarted {
        root: u32,
    },
    NodeInserted {
        node_id: u32,
        parent: Option<u32>,
        key: Option<u32>,
        sequence_index: Option<u32>,
        kind: i32,
        sem_type: i32,
        tag: String,
        value: String,
        graph_event: Option<crate::stream::StreamingEvent>,
    },
    KeyInserted {
        key_id: u32,
        parent: u32,
        key_text: String,
        tag: String,
    },
    NodeSealed {
        node_id: u32,
        graph_event: Option<crate::stream::StreamingEvent>,
    },
    DiagnosticAdded,
    DocumentEnded {
        root: u32,
    },
}

#[derive(Debug)]
struct RealEventTreeNode {
    kind: i32,
    sem_type: i32,
    tag: String,
    value: String,
    children: Vec<u32>,
    sealed: bool,
}

#[derive(Debug, Clone, Copy)]
struct RealEventTreeFrame {
    node_id: u32,
    kind: i32,
    pending_key: Option<u32>,
}

#[derive(Debug, Default)]
struct RealEventTreeStore {
    root: Option<u32>,
    nodes: Vec<RealEventTreeNode>,
    stack: Vec<RealEventTreeFrame>,
    ended: bool,
}

impl RealEventTreeStore {
    fn apply(&mut self, patch: &RealEventTreePatch) {
        assert!(
            !self.ended,
            "tree patch after DocumentEnded must be rejected"
        );
        match patch {
            RealEventTreePatch::DocumentStarted { root } => {
                assert!(self.root.is_none(), "DocumentStarted must be emitted once");
                assert!(self.nodes.is_empty(), "DocumentStarted must be first");
                self.root = Some(*root);
            }
            RealEventTreePatch::NodeInserted {
                node_id,
                parent,
                key,
                sequence_index,
                kind,
                sem_type,
                tag,
                value,
                ..
            } => {
                assert_eq!(
                    *node_id as usize,
                    self.nodes.len(),
                    "NodeId must be append-only"
                );
                match parent {
                    Some(parent_id) => {
                        self.attach_value(*parent_id, *node_id, *key, *sequence_index)
                    }
                    None => {
                        assert_eq!(
                            self.root,
                            Some(*node_id),
                            "root id must match DocumentStarted"
                        );
                        assert!(key.is_none(), "root must not carry key");
                        assert!(
                            sequence_index.is_none(),
                            "root must not carry sequence_index"
                        );
                    }
                }
                self.nodes.push(RealEventTreeNode {
                    kind: *kind,
                    sem_type: *sem_type,
                    tag: tag.clone(),
                    value: value.clone(),
                    children: Vec::new(),
                    sealed: false,
                });
                if matches!(*kind, 0 | 1) {
                    self.stack.push(RealEventTreeFrame {
                        node_id: *node_id,
                        kind: *kind,
                        pending_key: None,
                    });
                }
            }
            RealEventTreePatch::KeyInserted {
                key_id,
                parent,
                key_text,
                tag,
                ..
            } => {
                assert_eq!(*key_id as usize, self.nodes.len(), "key id must append");
                let frame = self
                    .stack
                    .last_mut()
                    .expect("map key must have open parent");
                assert_eq!(
                    frame.node_id, *parent,
                    "map key parent must be current frame"
                );
                assert_eq!(frame.kind, 1, "map key parent must be mapping");
                assert!(frame.pending_key.is_none(), "pending key must be empty");
                frame.pending_key = Some(*key_id);
                self.nodes.push(RealEventTreeNode {
                    kind: 2,
                    sem_type: 2,
                    tag: tag.clone(),
                    value: key_text.clone(),
                    children: Vec::new(),
                    sealed: true,
                });
            }
            RealEventTreePatch::NodeSealed { node_id, .. } => {
                let node = self
                    .nodes
                    .get_mut(*node_id as usize)
                    .expect("sealed node must exist");
                assert!(!node.sealed, "node must seal once");
                node.sealed = true;
                if matches!(node.kind, 0 | 1) {
                    let frame = self.stack.pop().expect("sealed container must be open");
                    assert_eq!(frame.node_id, *node_id, "sealed node must be stack top");
                    assert!(
                        frame.pending_key.is_none(),
                        "mapping cannot seal with pending key"
                    );
                }
            }
            RealEventTreePatch::DiagnosticAdded { .. } => {}
            RealEventTreePatch::DocumentEnded { root, .. } => {
                assert_eq!(self.root, Some(*root), "DocumentEnded root must match");
                assert!(self.stack.is_empty(), "all containers must be sealed");
                assert!(
                    self.nodes.iter().all(|node| node.sealed),
                    "all nodes must be sealed"
                );
                self.ended = true;
            }
        }
    }

    fn attach_value(
        &mut self,
        parent_id: u32,
        node_id: u32,
        key: Option<u32>,
        sequence_index: Option<u32>,
    ) {
        let parent = self
            .nodes
            .get_mut(parent_id as usize)
            .expect("parent must exist");
        assert!(!parent.sealed, "cannot append child to sealed parent");
        match parent.kind {
            0 => {
                assert!(key.is_none(), "sequence child must not carry key");
                assert_eq!(Some(parent.children.len() as u32), sequence_index);
                parent.children.push(node_id);
            }
            1 => {
                let frame = self.stack.last_mut().expect("mapping must have frame");
                assert_eq!(frame.node_id, parent_id, "mapping value parent mismatch");
                let pending_key = frame
                    .pending_key
                    .take()
                    .expect("mapping value must consume key");
                assert_eq!(Some(pending_key), key, "mapping value must use pending key");
                parent.children.push(pending_key);
                parent.children.push(node_id);
            }
            _ => panic!("only containers can receive children"),
        }
    }

    fn next_attachment(&self) -> (Option<u32>, Option<u32>, Option<u32>) {
        let Some(frame) = self.stack.last() else {
            return (None, None, None);
        };
        match frame.kind {
            0 => {
                let parent = self
                    .nodes
                    .get(frame.node_id as usize)
                    .expect("sequence parent must exist");
                (
                    Some(frame.node_id),
                    None,
                    Some(parent.children.len() as u32),
                )
            }
            1 => (Some(frame.node_id), frame.pending_key, None),
            _ => unreachable!(),
        }
    }

    fn current_container(&self, expected_kind: i32) -> u32 {
        let frame = self.stack.last().expect("container must be open");
        assert_eq!(frame.kind, expected_kind, "container kind mismatch");
        frame.node_id
    }

    fn materialize(&self) -> DocumentTreeNode {
        assert!(self.ended, "tree can only materialize after DocumentEnded");
        let root = self.root.expect("root must exist");
        normalize_tree(&self.materialize_node(root))
    }

    fn materialize_node(&self, node_id: u32) -> DocumentTreeNode {
        let node = &self.nodes[node_id as usize];
        DocumentTreeNode {
            kind: node.kind,
            sem_type: node.sem_type,
            tag: node.tag.clone(),
            value: node.value.clone(),
            children: node
                .children
                .iter()
                .map(|child| self.materialize_node(*child))
                .collect(),
        }
    }
}

#[derive(Default)]
struct RealEventTreeBuilder {
    store: RealEventTreeStore,
}

impl RealEventTreeBuilder {
    fn apply_event(&mut self, event: &crate::stream::StreamingEvent) -> Vec<RealEventTreePatch> {
        let mut patches = Vec::new();
        match event {
            crate::stream::StreamingEvent::DocStart(_) => {}
            crate::stream::StreamingEvent::DocEnd(_) => {
                let root = self.store.root.expect("DocEnd must follow root");
                patches.push(RealEventTreePatch::DocumentEnded { root });
            }
            crate::stream::StreamingEvent::MapStart(meta) => {
                let node_id = self.store.nodes.len() as u32;
                if self.store.root.is_none() {
                    patches.push(RealEventTreePatch::DocumentStarted { root: node_id });
                }
                let (parent, key, sequence_index) = self.store.next_attachment();
                patches.push(RealEventTreePatch::NodeInserted {
                    node_id,
                    parent,
                    key,
                    sequence_index,
                    kind: 1,
                    sem_type: 0,
                    tag: if meta.tag.is_empty() {
                        "!!map".to_owned()
                    } else {
                        meta.tag.clone()
                    },
                    value: String::new(),
                    graph_event: Some(event.clone()),
                });
            }
            crate::stream::StreamingEvent::MapKey { value, meta } => {
                let parent = self.store.current_container(1);
                patches.push(RealEventTreePatch::KeyInserted {
                    key_id: self.store.nodes.len() as u32,
                    parent,
                    key_text: value.clone(),
                    tag: if meta.tag.is_empty() {
                        "!!str".to_owned()
                    } else {
                        meta.tag.clone()
                    },
                });
            }
            crate::stream::StreamingEvent::MapEnd(_) => {
                let node_id = self.store.current_container(1);
                patches.push(RealEventTreePatch::NodeSealed {
                    node_id,
                    graph_event: Some(event.clone()),
                });
            }
            crate::stream::StreamingEvent::SeqStart(meta) => {
                let node_id = self.store.nodes.len() as u32;
                if self.store.root.is_none() {
                    patches.push(RealEventTreePatch::DocumentStarted { root: node_id });
                }
                let (parent, key, sequence_index) = self.store.next_attachment();
                patches.push(RealEventTreePatch::NodeInserted {
                    node_id,
                    parent,
                    key,
                    sequence_index,
                    kind: 0,
                    sem_type: 1,
                    tag: if meta.tag.is_empty() {
                        "!!seq".to_owned()
                    } else {
                        meta.tag.clone()
                    },
                    value: String::new(),
                    graph_event: Some(event.clone()),
                });
            }
            crate::stream::StreamingEvent::SeqEnd(_) => {
                let node_id = self.store.current_container(0);
                patches.push(RealEventTreePatch::NodeSealed {
                    node_id,
                    graph_event: Some(event.clone()),
                });
            }
            crate::stream::StreamingEvent::Scalar { value, meta } => {
                let node_id = self.store.nodes.len() as u32;
                if self.store.root.is_none() {
                    patches.push(RealEventTreePatch::DocumentStarted { root: node_id });
                }
                let (parent, key, sequence_index) = self.store.next_attachment();
                let sem_type = match meta.sem_type {
                    Some(crate::language::SemType::Map) => 0,
                    Some(crate::language::SemType::Seq) => 1,
                    Some(crate::language::SemType::Str) => 2,
                    Some(crate::language::SemType::Int) => 3,
                    Some(crate::language::SemType::Float) => 4,
                    Some(crate::language::SemType::Boolean) => 5,
                    Some(crate::language::SemType::Nil) => 6,
                    None => 2,
                };
                let rendered_value = if sem_type == 6 {
                    String::new()
                } else {
                    value.clone()
                };
                patches.push(RealEventTreePatch::NodeInserted {
                    node_id,
                    parent,
                    key,
                    sequence_index,
                    kind: 2,
                    sem_type,
                    tag: if meta.tag.is_empty() {
                        match sem_type {
                            0 => "!!map".to_owned(),
                            1 => "!!seq".to_owned(),
                            2 => "!!str".to_owned(),
                            3 => "!!int".to_owned(),
                            4 => "!!float".to_owned(),
                            5 => "!!bool".to_owned(),
                            6 => "!!null".to_owned(),
                            _ => "!!str".to_owned(),
                        }
                    } else {
                        meta.tag.clone()
                    },
                    value: rendered_value,
                    graph_event: Some(event.clone()),
                });
                patches.push(RealEventTreePatch::NodeSealed {
                    node_id,
                    graph_event: None,
                });
            }
            crate::stream::StreamingEvent::Alias { anchor, meta } => {
                let node_id = self.store.nodes.len() as u32;
                if self.store.root.is_none() {
                    patches.push(RealEventTreePatch::DocumentStarted { root: node_id });
                }
                let (parent, key, sequence_index) = self.store.next_attachment();
                patches.push(RealEventTreePatch::NodeInserted {
                    node_id,
                    parent,
                    key,
                    sequence_index,
                    kind: 3,
                    sem_type: -1,
                    tag: meta.tag.clone(),
                    value: anchor.clone(),
                    graph_event: Some(event.clone()),
                });
                patches.push(RealEventTreePatch::NodeSealed {
                    node_id,
                    graph_event: None,
                });
            }
            crate::stream::StreamingEvent::ParseError { .. } => {
                patches.push(RealEventTreePatch::DiagnosticAdded);
            }
        }
        for patch in &patches {
            self.store.apply(patch);
        }
        patches
    }

    fn materialize(&self) -> DocumentTreeNode {
        self.store.materialize()
    }
}

fn collect_real_event_batches(
    language: &str,
    chunks: &[&str],
) -> Vec<Vec<crate::stream::StreamingEvent>> {
    match language {
        "json" => {
            let mut parser =
                crate::stream::streaming_json::StreamingParser::with_path_emission(false, true);
            let mut batches = Vec::new();
            for chunk in chunks {
                parser.feed(chunk).expect("json chunk should decode");
                batches.push(parser.take_events());
            }
            batches.push(parser.finish().expect("json stream should finish"));
            batches
        }
        _ => panic!("unsupported language {language}"),
    }
}

type RealEventNodeIdentity = (String, i32, u32);
type RealEventEdgeIdentity = (String, i32, String, i32, i32, i32);

fn real_event_node_identity(node: &CanonicalGraphNode) -> RealEventNodeIdentity {
    (graph_path_to_string(&node.path), node.kind, node.depth)
}

fn real_event_edge_identity(edge: &CanonicalGraphEdge) -> RealEventEdgeIdentity {
    (
        graph_path_to_string(&edge.from_path),
        edge.from_row,
        graph_path_to_string(&edge.to_path),
        edge.to_row,
        edge.from_kind,
        edge.to_kind,
    )
}

fn real_event_cell_sem_eq(left: &CanonicalCell, right: &CanonicalCell) -> bool {
    left.sem_type == right.sem_type
        && left.path == right.path
        && left.text == right.text
        && left.value == right.value
        && left.format_text == right.format_text
}

fn real_event_row_sem_eq(left: &CanonicalRow, right: &CanonicalRow) -> bool {
    left.index == right.index
        && left.cells.len() == right.cells.len()
        && left
            .cells
            .iter()
            .zip(right.cells.iter())
            .all(|(left, right)| real_event_cell_sem_eq(left, right))
}

fn real_event_strip_table_node(node: &CanonicalGraphNode) -> CanonicalGraphNode {
    let mut stripped = node.clone();
    if let Some(table) = node.table.as_ref() {
        stripped.table = Some(CanonicalTable {
            columns: Vec::new(),
            rows: Vec::new(),
            header_height: table.header_height,
            total_height: table.total_height,
            view_height: table.view_height,
            row_height: table.row_height,
        });
    }
    stripped
}

#[derive(Debug, Clone)]
enum RealEventTablePatch {
    TableCreated {
        table_handle: u32,
        columns: Vec<CanonicalCell>,
        header_height: i32,
        total_height: i32,
        view_height: i32,
        row_height: i32,
    },
    ColumnsReplaced {
        table_handle: u32,
        columns: Vec<CanonicalCell>,
    },
    RowsAppended {
        table_handle: u32,
        start_index: usize,
        rows: Vec<CanonicalRow>,
    },
    RowsReplaced {
        table_handle: u32,
        start_index: usize,
        rows: Vec<CanonicalRow>,
    },
}

#[derive(Debug, Clone, Default)]
struct RealEventGraphPatch {
    nodes_added: Vec<(u32, CanonicalGraphNode)>,
    nodes_updated: Vec<(u32, CanonicalGraphNode)>,
    nodes_removed: Vec<u32>,
    edges_added: Vec<CanonicalGraphEdge>,
    edges_removed: Vec<RealEventEdgeIdentity>,
    table_patches: Vec<RealEventTablePatch>,
}

impl RealEventGraphPatch {
    fn is_empty(&self) -> bool {
        self.nodes_added.is_empty()
            && self.nodes_updated.is_empty()
            && self.nodes_removed.is_empty()
            && self.edges_added.is_empty()
            && self.edges_removed.is_empty()
            && self.table_patches.is_empty()
    }
}

#[derive(Debug, Clone)]
struct RealEventProjectionPatch {
    clear: bool,
    graph_data: RealEventGraphPatch,
    patch_seq: u64,
    base_graph_version: u64,
    graph_version: u64,
}

struct RealEventGraphProjector {
    previous: CanonicalGraphView,
    handles: BTreeMap<RealEventNodeIdentity, u32>,
    next_handle: u32,
    patch_seq: u64,
    graph_version: u64,
}

impl Default for RealEventGraphProjector {
    fn default() -> Self {
        Self {
            previous: CanonicalGraphView {
                nodes: Vec::new(),
                edges: Vec::new(),
            },
            handles: BTreeMap::new(),
            next_handle: 0,
            patch_seq: 0,
            graph_version: 0,
        }
    }
}

impl RealEventGraphProjector {
    fn handle_for(&mut self, identity: &RealEventNodeIdentity) -> u32 {
        if let Some(handle) = self.handles.get(identity) {
            *handle
        } else {
            let handle = self.next_handle;
            self.next_handle += 1;
            self.handles.insert(identity.clone(), handle);
            handle
        }
    }

    fn apply_tree_patch(
        &mut self,
        patch: &RealEventTreePatch,
        current: &CanonicalGraphView,
    ) -> Option<RealEventProjectionPatch> {
        if !real_event_patch_has_graph_effect(patch) {
            return None;
        }
        let graph_data = self.diff(current);
        self.previous = current.clone();
        if graph_data.is_empty() {
            return None;
        }
        let base_graph_version = self.graph_version;
        self.graph_version += 1;
        self.patch_seq += 1;
        Some(RealEventProjectionPatch {
            clear: base_graph_version == 0,
            graph_data,
            patch_seq: self.patch_seq,
            base_graph_version,
            graph_version: self.graph_version,
        })
    }

    fn diff(&mut self, current: &CanonicalGraphView) -> RealEventGraphPatch {
        let old_nodes = self
            .previous
            .nodes
            .iter()
            .map(|node| (real_event_node_identity(node), node.clone()))
            .collect::<BTreeMap<_, _>>();
        let new_nodes = current
            .nodes
            .iter()
            .map(|node| (real_event_node_identity(node), node.clone()))
            .collect::<BTreeMap<_, _>>();

        let mut patch = RealEventGraphPatch::default();

        for identity in old_nodes.keys() {
            if !new_nodes.contains_key(identity) {
                patch.nodes_removed.push(self.handle_for(identity));
            }
        }

        for (identity, new_node) in &new_nodes {
            let handle = self.handle_for(identity);
            match old_nodes.get(identity) {
                None => self.push_added_node(handle, new_node, &mut patch),
                Some(old_node) => self.push_updated_node(handle, old_node, new_node, &mut patch),
            }
        }

        let old_edges = self
            .previous
            .edges
            .iter()
            .map(|edge| (real_event_edge_identity(edge), edge.clone()))
            .collect::<BTreeMap<_, _>>();
        let new_edges = current
            .edges
            .iter()
            .map(|edge| (real_event_edge_identity(edge), edge.clone()))
            .collect::<BTreeMap<_, _>>();

        for identity in old_edges.keys() {
            if !new_edges.contains_key(identity) {
                patch.edges_removed.push(identity.clone());
            }
        }
        for (identity, edge) in &new_edges {
            if !old_edges.contains_key(identity) {
                patch.edges_added.push((*edge).clone());
            }
        }

        patch
    }

    fn push_added_node(
        &mut self,
        handle: u32,
        node: &CanonicalGraphNode,
        patch: &mut RealEventGraphPatch,
    ) {
        if let Some(table) = node.table.as_ref() {
            patch
                .nodes_added
                .push((handle, real_event_strip_table_node(node)));
            patch.table_patches.push(RealEventTablePatch::TableCreated {
                table_handle: handle,
                columns: table.columns.clone(),
                header_height: table.header_height,
                total_height: table.total_height,
                view_height: table.view_height,
                row_height: table.row_height,
            });
            if !table.rows.is_empty() {
                patch.table_patches.push(RealEventTablePatch::RowsAppended {
                    table_handle: handle,
                    start_index: 0,
                    rows: table.rows.clone(),
                });
            }
        } else {
            patch.nodes_added.push((handle, node.clone()));
        }
    }

    fn push_updated_node(
        &mut self,
        handle: u32,
        old_node: &CanonicalGraphNode,
        new_node: &CanonicalGraphNode,
        patch: &mut RealEventGraphPatch,
    ) {
        match (old_node.table.as_ref(), new_node.table.as_ref()) {
            (Some(old_table), Some(new_table)) => {
                if old_table.columns.len() != new_table.columns.len()
                    || !old_table
                        .columns
                        .iter()
                        .zip(new_table.columns.iter())
                        .all(|(left, right)| real_event_cell_sem_eq(left, right))
                {
                    patch
                        .table_patches
                        .push(RealEventTablePatch::ColumnsReplaced {
                            table_handle: handle,
                            columns: new_table.columns.clone(),
                        });
                }

                let common_len = old_table.rows.len().min(new_table.rows.len());
                let mut replace_start = None;
                for index in 0..common_len {
                    if !real_event_row_sem_eq(&old_table.rows[index], &new_table.rows[index]) {
                        replace_start = Some(index);
                        break;
                    }
                }
                if let Some(start_index) = replace_start {
                    patch.table_patches.push(RealEventTablePatch::RowsReplaced {
                        table_handle: handle,
                        start_index,
                        rows: new_table.rows[start_index..common_len].to_vec(),
                    });
                }
                if new_table.rows.len() > old_table.rows.len() {
                    patch.table_patches.push(RealEventTablePatch::RowsAppended {
                        table_handle: handle,
                        start_index: old_table.rows.len(),
                        rows: new_table.rows[old_table.rows.len()..].to_vec(),
                    });
                }
            }
            (None, Some(_)) => {
                patch.nodes_removed.push(handle);
                self.push_added_node(handle, new_node, patch);
            }
            (Some(_), None) => {
                patch.nodes_updated.push((handle, new_node.clone()));
            }
            (None, None) => {
                if old_node != new_node {
                    patch.nodes_updated.push((handle, new_node.clone()));
                }
            }
        }
    }
}

fn real_event_patch_has_graph_effect(patch: &RealEventTreePatch) -> bool {
    match patch {
        RealEventTreePatch::DocumentStarted { .. } => false,
        RealEventTreePatch::NodeInserted { graph_event, .. }
        | RealEventTreePatch::NodeSealed { graph_event, .. } => graph_event.is_some(),
        RealEventTreePatch::KeyInserted { .. }
        | RealEventTreePatch::DiagnosticAdded { .. }
        | RealEventTreePatch::DocumentEnded { .. } => true,
    }
}

#[derive(Debug, Default)]
struct RealEventGraphRenderState {
    version: u64,
    nodes_by_handle: BTreeMap<u32, CanonicalGraphNode>,
    edges_by_identity: BTreeMap<RealEventEdgeIdentity, CanonicalGraphEdge>,
}

impl RealEventGraphRenderState {
    fn apply(&mut self, patch: &RealEventProjectionPatch) -> Result<(), String> {
        if patch.base_graph_version != self.version {
            return Err(format!(
                "base graph version mismatch: state={}, patch={}",
                self.version, patch.base_graph_version
            ));
        }
        if patch.clear {
            self.nodes_by_handle.clear();
            self.edges_by_identity.clear();
        }
        for handle in &patch.graph_data.nodes_removed {
            self.nodes_by_handle.remove(handle);
        }
        for (handle, node) in &patch.graph_data.nodes_added {
            self.nodes_by_handle.insert(*handle, node.clone());
        }
        for (handle, node) in &patch.graph_data.nodes_updated {
            if !self.nodes_by_handle.contains_key(handle) {
                return Err(format!("node {} updated before add", handle));
            }
            self.nodes_by_handle.insert(*handle, node.clone());
        }
        for identity in &patch.graph_data.edges_removed {
            self.edges_by_identity.remove(identity);
        }
        for edge in &patch.graph_data.edges_added {
            self.edges_by_identity
                .insert(real_event_edge_identity(edge), edge.clone());
        }
        for table_patch in &patch.graph_data.table_patches {
            self.apply_table_patch(table_patch)?;
        }
        self.version = patch.graph_version;
        Ok(())
    }

    fn apply_table_patch(&mut self, patch: &RealEventTablePatch) -> Result<(), String> {
        match patch {
            RealEventTablePatch::TableCreated {
                table_handle,
                columns,
                header_height,
                total_height,
                view_height,
                row_height,
            } => {
                let node = self
                    .nodes_by_handle
                    .get_mut(table_handle)
                    .ok_or_else(|| format!("table node {} must exist", table_handle))?;
                node.table = Some(CanonicalTable {
                    columns: columns.clone(),
                    rows: Vec::new(),
                    header_height: *header_height,
                    total_height: *total_height,
                    view_height: *view_height,
                    row_height: *row_height,
                });
            }
            RealEventTablePatch::ColumnsReplaced {
                table_handle,
                columns,
            } => {
                let table = self.table_mut(*table_handle)?;
                table.columns = columns.clone();
            }
            RealEventTablePatch::RowsAppended {
                table_handle,
                start_index,
                rows,
            } => {
                let table = self.table_mut(*table_handle)?;
                if table.rows.len() != *start_index {
                    return Err(format!(
                        "row append gap for table {}: len={}, start={}",
                        table_handle,
                        table.rows.len(),
                        start_index
                    ));
                }
                table.rows.extend(rows.iter().cloned());
            }
            RealEventTablePatch::RowsReplaced {
                table_handle,
                start_index,
                rows,
            } => {
                let table = self.table_mut(*table_handle)?;
                let end = start_index + rows.len();
                if end > table.rows.len() {
                    return Err(format!(
                        "row replace out of range for table {}: len={}, end={}",
                        table_handle,
                        table.rows.len(),
                        end
                    ));
                }
                for (offset, row) in rows.iter().enumerate() {
                    table.rows[start_index + offset] = row.clone();
                }
            }
        }
        Ok(())
    }

    fn table_mut(&mut self, handle: u32) -> Result<&mut CanonicalTable, String> {
        let node = self
            .nodes_by_handle
            .get_mut(&handle)
            .ok_or_else(|| format!("table node {} must exist", handle))?;
        node.table
            .as_mut()
            .ok_or_else(|| format!("table node {} missing table", handle))
    }

    fn materialize(&self) -> CanonicalGraphView {
        let mut nodes = self.nodes_by_handle.values().cloned().collect::<Vec<_>>();
        nodes.sort_by(|left, right| {
            canonical_node_sort_key(left).cmp(&canonical_node_sort_key(right))
        });
        let mut edges = self.edges_by_identity.values().cloned().collect::<Vec<_>>();
        edges.sort_by(|left, right| {
            canonical_edge_sort_key(left).cmp(&canonical_edge_sort_key(right))
        });
        CanonicalGraphView { nodes, edges }
    }
}

struct RealEventDemoArtifacts {
    tree: DocumentTreeNode,
    graph: CanonicalGraphView,
    tree_patches: Vec<RealEventTreePatch>,
    graph_patches: Vec<RealEventProjectionPatch>,
}

fn current_canonical_graph_from_builder(
    builder: &crate::stream::TreeBuilder,
    language: &str,
) -> CanonicalGraphView {
    let Some((store, root)) = builder.snapshot_tree() else {
        return CanonicalGraphView {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
    };
    let delta = crate::graph::graph_projection_service::build_initial_projection_delta(
        &store, root, language, None,
    );
    canonical_graph_view(&delta)
}

fn run_real_event_patch_demo(language: &str, chunks: &[&str]) -> RealEventDemoArtifacts {
    let event_batches = collect_real_event_batches(language, chunks);
    let mut patch_builder = RealEventTreeBuilder::default();
    let mut real_tree_builder = crate::stream::TreeBuilder::new();
    let mut graph_projector = RealEventGraphProjector::default();
    let mut graph_render_state = RealEventGraphRenderState::default();
    let mut tree_patches = Vec::new();
    let mut graph_patches = Vec::new();

    for batch in event_batches {
        for event in batch {
            real_tree_builder
                .push(&event)
                .expect("real tree builder should accept event");
            let current_graph = current_canonical_graph_from_builder(&real_tree_builder, language);
            for patch in patch_builder.apply_event(&event) {
                if let Some(graph_patch) = graph_projector.apply_tree_patch(&patch, &current_graph)
                {
                    graph_render_state
                        .apply(&graph_patch)
                        .expect("graph patch should apply to render state");
                    graph_patches.push(graph_patch);
                }
                tree_patches.push(patch);
            }
        }
    }

    RealEventDemoArtifacts {
        tree: patch_builder.materialize(),
        graph: graph_render_state.materialize(),
        tree_patches,
        graph_patches,
    }
}

fn assert_real_event_graph_semantics_match(
    actual: &CanonicalGraphView,
    expected: &CanonicalGraphView,
) {
    let actual_summary = summarize_graph(actual);
    let expected_summary = summarize_graph(expected);
    assert_eq!(
        actual_summary.semantic_fingerprint,
        expected_summary.semantic_fingerprint
    );
    assert_eq!(actual_summary.node_count, expected_summary.node_count);
    assert_eq!(actual_summary.edge_count, expected_summary.edge_count);
    assert_eq!(
        actual_summary.node_kind_counts,
        expected_summary.node_kind_counts
    );
    assert_eq!(
        actual_summary.total_table_rows,
        expected_summary.total_table_rows
    );
    assert_eq!(
        actual_summary.table_node_count,
        expected_summary.table_node_count
    );
}

fn assert_real_event_patch_sequence(patches: &[RealEventProjectionPatch]) {
    assert!(!patches.is_empty(), "graph patch stream must not be empty");
    let mut expected_version = 0;
    for (index, patch) in patches.iter().enumerate() {
        assert_eq!(
            patch.patch_seq,
            index as u64 + 1,
            "patch_seq must be monotonic"
        );
        assert_eq!(
            patch.base_graph_version, expected_version,
            "base_graph_version must chain"
        );
        expected_version += 1;
        assert_eq!(
            patch.graph_version, expected_version,
            "graph_version must advance by one"
        );
    }
}

fn assert_real_event_version_mismatch_is_rejected(patches: &[RealEventProjectionPatch]) {
    assert!(patches.len() > 2, "need enough patches to skip one");
    let mut state = RealEventGraphRenderState::default();
    state.apply(&patches[0]).expect("first patch should apply");
    assert!(
        state.apply(&patches[2]).is_err(),
        "skipping a patch must be rejected"
    );
}

#[test]
fn wasm_document_real_event_patch_demo_json_matches_existing_snapshot_semantics() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = load_fixture("example/simple.json");
    let split = split_before_needle(&source, r#"    {"h1": 21"#);
    let chunks = [&source[..split], &source[split..]];
    let expected = final_artifacts("simple-json-real-event-demo", "json", &[&source]);
    let demo = run_real_event_patch_demo("json", &chunks);

    let replayed_tree = {
        let mut store = RealEventTreeStore::default();
        for patch in &demo.tree_patches {
            store.apply(patch);
        }
        store.materialize()
    };
    assert_eq!(demo.tree, expected.tree);
    assert_eq!(replayed_tree, expected.tree);
    assert_real_event_graph_semantics_match(&demo.graph, &expected.graph);
    assert_real_event_patch_sequence(&demo.graph_patches);
    assert_real_event_version_mismatch_is_rejected(&demo.graph_patches);
    insta::assert_yaml_snapshot!("streaming_simple_json_final_artifacts", expected.clone());
}

#[test]
fn wasm_document_real_event_patch_demo_large_json_table_uses_linear_row_patches() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = build_json_table_document(64);
    let first = split_at_or_after_char_boundary(&source, source.len() / 3);
    let second = split_at_or_after_char_boundary(&source, source.len() * 2 / 3);
    let chunks = chunk_slices(&source, &[first, second]);
    let expected = final_artifacts("large-json-table-real-event-demo", "json", &[&source]);
    let demo = run_real_event_patch_demo("json", &chunks);

    assert_eq!(demo.tree, expected.tree);
    assert_real_event_graph_semantics_match(&demo.graph, &expected.graph);
    assert_real_event_patch_sequence(&demo.graph_patches);
    assert_real_event_version_mismatch_is_rejected(&demo.graph_patches);

    let rows_appended: usize = demo
        .graph_patches
        .iter()
        .flat_map(|patch| patch.graph_data.table_patches.iter())
        .filter_map(|patch| match patch {
            RealEventTablePatch::RowsAppended { rows, .. } => Some(rows.len()),
            _ => None,
        })
        .sum();
    let final_rows: usize = expected
        .graph
        .nodes
        .iter()
        .filter_map(|node| node.table.as_ref())
        .map(|table| table.rows.len())
        .sum();
    let table_node_updates = demo
        .graph_patches
        .iter()
        .flat_map(|patch| patch.graph_data.nodes_updated.iter())
        .filter(|(_handle, node)| node.table.is_some())
        .count();

    assert_eq!(
        rows_appended, final_rows,
        "row append payload should stay linear"
    );
    assert_eq!(
        table_node_updates, 0,
        "table rows should not fall back to nodes_updated"
    );
}

#[test]
fn wasm_document_escape_nest_json_reconstructs_original_graph_when_enabled() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = load_fixture("test/fixtures/json/1MB-min.1.json");

    // Graph A: parse the original JSON normally
    let baseline = final_artifacts("escape-nest-1mb-original", "json", &[&source]);

    // Escape: wrap the entire source text as a JSON string value, so the result
    // is itself a valid JSON document containing only a single string.
    // Example: [1,2] → "[1,2]" (a JSON string whose content is the original JSON).
    let escaped = serde_json::to_string(&source)
        .expect("source text should serialize as a JSON string value");

    // Graph B: feed the escaped JSON string as the source to a streaming job
    // with nest mode enabled. Nested expansion should reconstruct the same
    // graph as the original document.
    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "escape-nest-1mb-nested".to_owned(),
        language: "json".to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: DocumentFormattingSettings::default(),
        },
    })
    .expect("nest-mode job should start");

    let _ = text_chunk(started.job_handle, &escaped);
    let close_batch = close(started.job_handle);

    assert!(
        matches!(close_batch.terminal, Some(JobTerminal::Completed)),
        "nest-mode parse of escaped JSON string should complete"
    );

    let snapshot = stored_snapshot_for_document("escape-nest-1mb-nested")
        .expect("nest-mode snapshot should exist");

    let nested_graph = snapshot
        .analysis
        .as_ref()
        .and_then(|analysis| analysis.document.as_ref())
        .map(|decoded| {
            canonical_graph_view(
                &crate::graph::graph_projection_service::build_initial_projection_delta(
                    &decoded.store,
                    decoded.root,
                    "json",
                    Some("escape-nest-1mb-nested"),
                ),
            )
        })
        .expect("nest-mode should produce a graph");

    assert_eq!(
        baseline.graph, nested_graph,
        "escape → nest-parse should reconstruct the original graph when nested expansion is enabled"
    );
}
