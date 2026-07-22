# Treease

Treease is a structured text workspace for inspecting, tracing, editing, comparing, and exporting JSON, YAML, TOML, CSV, and embedded payloads.

## Why Treease

Raw structured text becomes hard to follow long before it becomes hard to edit. Treease keeps source text and graph context attached to the same document state so you can see the structure first, then trace fields, review changes, compare results, and export with confidence.

Instead of switching between a plain editor, an isolated viewer, and one-off conversion commands, Treease keeps the document, its visual structure, and the next action in the same workflow.

## Core Capabilities

- Open a local structured file and see its real shape without leaving the source.
- Trace a field across graph, tree path, and source text without losing place.
- Format, minify, sort keys, and edit while keeping visual context attached to the same document.
- Compare structure before trusting a text diff, and preview converted output before export.
- Open reproducible editor or viewer states from URL-backed presets when sharing examples, demos, or bug reports.
- Query, convert, and visualize structured documents from the CLI, including a readonly local web graph view.

## Quick Start

### Web

```bash
pnpm install
pnpm dev
```

### CLI

```bash
cargo install treease-cli

# Quick examples
treease '.services.api.url' example.json
treease -o yaml '.' example.json
treease web '.services.api' example.json

# Read a value
treease '.a.b' file.yaml

# Convert one format to another
treease -p yaml -o json '.' file.yaml

# Write a change back to the file
treease -i '.a = 1' file.yaml

# Inspect CLI capabilities
treease help --format json
treease operators list
treease formats list --format json
treease examples search "filter array"
treease doctor --format json
```

## Web CLI Configuration

`treease web` opens the hosted `/editor` page and keeps the input data in a short-lived localhost service. Set `TREEASE_WEB_URL` when using a self-hosted Web deployment; it should point to that deployment's `/editor` route.

## Development

### Environment Files

Each app owns its environment configuration. Copy the relevant `.env.example`
to `.env.local` for local development; keep `.env.local` untracked and never
put server secrets in `apps/web` configuration. Web tests may use the committed
`apps/web/.env.test` defaults, with private overrides in `.env.test.local`.

The server uses `apps/server/.env.local` for both Node.js and local Wrangler
development. Wrangler-only local values may instead be placed in its ignored
local vars file, based on `apps/server/.dev.vars.example`. Production values
belong in the deployment platform's variables and secrets, not in `.env` files.

### Repository Layout

- `apps/web/`: Svelte web application, editor UI, graph UI, and browser worker boundary.
- `apps/cli/`: standalone CLI crate, acceptance tests, and CLI metadata export.
- `packages/core/`: Rust parser, formatter, operators, evaluation, graph build, snapshots, and WASM exports.

### Common Commands

```bash
cd apps/web
pnpm dev
pnpm build
pnpm test
pnpm test:unit
pnpm test:integration
pnpm test:e2e

cd ../../packages/core
cargo nextest run --locked

cd ../../apps/cli
cargo nextest run --locked --lib
bash tests/acceptance/run.sh
```

### Local `treease web` Testing

Set `TREEASE_WEB_URL` to a locally running Web application's `/editor` route, then run `treease web`. The CLI still serves only the input source from localhost; the Web application supplies the page assets.

### Protocol and WASM Regeneration

When changing the document protocol or Rust/WASM boundary:

```bash
cd packages/core
cargo run --locked --bin export_document_protocol

cd ../../apps/web
pnpm wasm:bindgen
pnpm wasm:sync
```

### Documentation Consistency

After changing documentation, run:

```bash
node scripts/check-docs.mjs
```
