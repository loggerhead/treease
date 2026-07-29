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

### Homebrew

On macOS, install the CLI or Desktop App from the Treease tap:

```bash
brew tap loggerhead/treease
brew install treease
brew install --cask treease
```

Release automation updates `loggerhead/homebrew-treease` after the crates.io
or GitHub release completes. Store a token that can dispatch workflows in the
tap as the `HOMEBREW_TAP_TOKEN` GitHub Actions secret.

## Web CLI Configuration

`treease web` opens the hosted `/editor` page and keeps the input data in a short-lived localhost service. Set `TREEASE_WEB_URL` when using a self-hosted Web deployment; it should point to that deployment's `/editor` route.

## Repository Boundary

This repository contains the public Web, Desktop, CLI, Core, and protocol packages.
The hosted API implementation for accounts, billing, sharing, AI, usage, and file
storage is maintained separately and is consumed over the documented HTTPS API
boundary.

## License

Treease is source-available under the [Treease Community License](LICENSE). It is
not an OSI-approved open source license. Review the license before using Treease
in a commercial product or hosted service.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development checks, scope boundaries,
and pull request expectations.

## Development

### Environment Files

Each app owns its environment configuration. Copy the relevant `.env.example`
to `.env.local` for local development; keep `.env.local` untracked and never
put service credentials in `apps/web` configuration. Web tests may use the committed
`apps/web/.env.test` defaults, with private overrides in `.env.test.local`.

The Web API origin is configured with `PUBLIC_API_ORIGIN`; it identifies the
separately deployable API and is not a credential.

Production service values belong in the private service repository's deployment
platform variables and secrets, not in this repository's `.env` files.

### Repository Layout

- `apps/web/`: Svelte web application, editor UI, graph UI, and browser worker boundary.
- `apps/cli/`: standalone CLI crate, acceptance tests, and CLI metadata export.
- `packages/core/`: Rust parser, formatter, operators, evaluation, graph build, snapshots, and WASM exports.
- `packages/api-contracts/`: public HTTP request/response schemas shared by Web and the separately deployable service.
- `packages/share-protocol/`: public serialized share-resource schema.

The hosted API is a separately deployable service configured through
`PUBLIC_API_ORIGIN`; its implementation and operational configuration are not part
of this repository.

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
