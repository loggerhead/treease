# Graph Boundary

This directory owns graph models, deterministic graph construction, projection, delta calculation, and relayout decisions derived from Core documents.

## Public Contracts

- Graph model and identity: `graph_model.rs`, `graph_identity.rs`, `graph_model_index.rs`
- Full construction: `graph_builder.rs`, `graph_builder/`, `graph_builder_preorder.rs`
- Incremental updates: `graph_delta.rs`, `graph_delta_service.rs`, `graph_fragment_index.rs`
- Projection and materialization: `graph_projection_service.rs`, `graph_materialize.rs`
- Layout and topology: `graph_relayout.rs`, `graph_shape.rs`, `graph_topology.rs`

## Boundary Rules

- Build graph structure from Core trees and snapshots; do not encode Web rendering or DOM concerns here.
- Preserve graph identity and delta semantics across full and incremental construction paths.
- Keep graph-model mutations, fragment impact analysis, and projection materialization in their named modules instead of bypassing them with ad hoc patches.
- Send layout engine mechanics to `../layout/`; this module decides graph-specific projection and relayout scope.
