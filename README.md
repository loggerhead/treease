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

## Asset Configuration

The Web app and `treease web` can fetch static assets from a remote asset origin. The default values in this repository point at Treease-managed infrastructure and are intended for the official deployment path, not as a guarantee that those endpoints or buckets are suitable for third-party use.

- `PUBLIC_ASSET_BASE_URL` controls the Web app asset origin.
- `TREEASE_R2_ASSET_BUCKET` is used by the asset upload/check scripts under `apps/web/scripts/`.
- The CLI build also has a default remote asset base URL for `treease web`.

For self-hosted or community deployments, set these values explicitly for your own environment instead of relying on the repository defaults.

## Development

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

When debugging the shared CLI/Web graph page locally, build the Web assets first and then point the CLI at the local `cli-assets` bundle instead of the public site:

```bash
pnpm --dir apps/web build
node ./scripts/treease-web-local.mjs . path/to/file.json
```

`node ./scripts/treease-web-local.mjs` starts a local static server for `apps/web/build/cli-assets`, runs the current checkout's CLI through `cargo run`, and injects an isolated `TREEASE_WEB_CACHE_DIR` under `.tmp/`. That avoids stale cache hits when `wasm_release_date` stays the same but the local bundle changes.

If you need the env vars for manual commands, run:

```bash
node ./scripts/treease-web-local.mjs serve
```

That prints the local `TREEASE_WEB_ASSET_BASE_URL` and a matching isolated cache path while keeping the static server running in the foreground.

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
