# Stream Boundary

This directory owns streaming decode events, event sinks, tree construction, and tree patches for incremental input.

## Public Contracts

- Streaming decode entry points and options: `streaming_decoder.rs`
- Event protocol: `streaming_events.rs`
- Tree construction: `tree_builder.rs`
- Incremental JSON support: `streaming_json.rs`
- Patch representation: `tree_patch.rs`

## Boundary Rules

- Normalize input through the streaming decoder APIs; do not add parallel format-specific stream loops in callers.
- Keep event ordering and event payload semantics compatible with `EventSink` and `TreeBuilder`.
- Route non-streaming decoding through the same decode contract when possible; do not fork document semantics by transport mode.
- Keep job lifecycle and snapshot publication in `../document/`, not in stream decoding.
