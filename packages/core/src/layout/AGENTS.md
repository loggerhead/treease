# Layout Boundary

This directory owns generic layout-engine integration and the adapter that turns graph layout requests into Core layout results.

## Public Contracts

- Layout engine: `layout_engine.rs`
- Full graph layout adapter: `full_layout_adapter.rs`
- Graph model consumers: `../graph/graph_model.rs`, `../graph/graph_projection_service.rs`

## Boundary Rules

- Keep generic layout mechanics here; graph-specific topology, projection, and relayout scope remain in `../graph/`.
- Preserve deterministic geometry and coordinate contracts expected by graph projection.
- Do not add DOM measurement, viewport state, or rendering policy here.
