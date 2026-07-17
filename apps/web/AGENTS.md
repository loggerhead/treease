---
summary: "apps/web layer boundaries, responsibilities, and verification constraints."
read_when:
  - Confirming whether a web change reaches the correct layer
---

# apps/web Guide

## Scope

- This directory owns frontend UI, interaction, Workers, static assets, and Web tests.

## Boundary Rules

- Do not implement Core computation—parsing, operators, evaluation, or formatting—here.
- All Core capability calls go through `src/workers` and `../../packages/core/wasm`.
- Keep `src/lib/components/GraphViewer.svelte` and `src/workers/wasm-runtime.worker.ts` thin shells.
- Change protocol fields only in `../../packages/core/src/document/protocol.rs`; never hand-edit generated output.

## Code Placement

- GraphViewer logic belongs in `src/lib/components/graph-viewer/` first.
- Worker runtime logic belongs in `src/workers/runtime/`; do not push it back into `src/workers/wasm-runtime.worker.ts`.
- Put unit tests in adjacent `**/*.test.ts` files first.
