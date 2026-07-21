# treease-cli

`treease-cli` is the command-line interface for Treease.

## Quick Start

```bash
treease '.a.b' file.yaml
treease -p yaml -o json '.' file.yaml
treease web '.services.api' config.yaml
```

## What It Includes

- Expression evaluation over structured documents
- Format conversion and in-place writes
- Operator and format discovery commands
- A hosted readonly Web graph UI backed by a short-lived localhost data service served by `treease web`

## Repository

- Source: <https://github.com/loggerhead/treease>
- Core crate: `treease-core`
- CLI source and usage: <https://github.com/loggerhead/treease/tree/main/apps/cli>

## Local Verification

```bash
cd apps/cli
cargo nextest run --locked --lib
bash tests/acceptance/run.sh
```
