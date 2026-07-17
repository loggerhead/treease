# Analysis Boundary

This directory owns document diagnostics, error spans, line indexing, and structural-span indexing derived from parsed documents.

## Public Contracts

- Diagnostics model and source locations: `diagnostics.rs`
- Document analysis demand and payloads: `document_analysis.rs`
- Line and column lookup: `line_index.rs`
- Structural source spans: `span_index.rs`
- WASM payload encoding: `../wasm/document_analysis_shared.rs`

## Boundary Rules

- Derive diagnostics from Core parsing and trees; do not reparse or recreate source-location logic in consumers.
- Keep internal analysis data separate from the WASM payload encoder.
- Preserve byte/line/column and span-index consistency when changing source analysis.
- Keep UI decorations and presentation policy outside this directory.
