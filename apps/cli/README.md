# treease-cli

`treease-cli` evaluates, transforms, formats, converts, and previews structured
documents from the terminal. It uses the same Treease core as the Web workspace
and is designed for JSON, YAML, TOML, CSV, and other supported structured
formats.

## Installation

Install the latest published CLI with Cargo:

```bash
cargo install treease-cli
```

On macOS, install the CLI from the Treease Homebrew tap:

```bash
brew tap loggerhead/treease
brew install treease
```

Check the installed version and the complete generated help with:

```bash
treease --help
treease help --format json
```

## Basic usage

The first positional argument is a Treease expression. The optional final
argument is an input file; when it is omitted, Treease reads standard input.

```bash
# Read a nested value from a YAML file.
treease '.services.api.url' config.yaml

# Read JSON from stdin and convert the result to YAML.
cat response.json | treease -o yaml '.'

# Select an input format and an output format explicitly.
treease -p yaml -o json '.' config.yaml

# Evaluate an expression without an input document.
treease --null-input '.hello = "world"'

# Write a result back to one input file.
treease --inplace '.version = "2"' package.json
```

The default expression is `.`. It returns the complete input document.

## Input and output options

| Option | Description |
| --- | --- |
| `-p, --input-format FORMAT` | Set the input format explicitly. |
| `-o, --output-format FORMAT` | Set the output format explicitly. |
| `-P, --prettyPrint` | Pretty-print the output. |
| `-I, --indent INDENT` | Set the output indentation width. |
| `-r, --unwrapScalar` | Print scalar results without a document wrapper. |
| `-N, --no-doc` | Disable YAML document separators. |
| `-i, --inplace` | Write the result back to the input file. Requires exactly one file. |
| `-n, --null-input` | Evaluate without reading a file or standard input. |
| `-e, --exit-status` | Exit with status 1 when the result is empty, null, or false. |

Use `treease formats list` to inspect the formats available in the installed
build. Use `-p` or `-o` when the file extension is missing or does not describe
the actual content.

## Web graph view

`treease web` evaluates one input document and opens a readonly Treease graph
view for the selected result:

```bash
treease web '.services.api' config.yaml
treease web '.' -
```

The command accepts one file path or `-` for standard input. It starts a
short-lived localhost data service and opens the configured Treease Web URL.
The Web graph view is readonly; use the normal `treease` command when the
result needs to be written or further transformed in the terminal.

## Operator and format discovery

The CLI exposes machine-readable discovery commands so scripts and tools can
inspect the installed capability set without duplicating the registry:

```bash
treease operators list
treease operators list --category string
treease operators get select
treease operators search map

treease formats list
treease formats get yaml
```

Add `--format json` to commands that support it:

```bash
treease operators list --format json
treease formats list --format json
```

## Common expression patterns

```bash
# Traverse a path.
treease '.services.api' config.yaml

# Select matching values.
treease '.users[] | select(.active == true)' users.json

# Convert a selected value.
treease -o yaml '.deployment' config.json

# Update a value in place.
treease --inplace '.metadata.version = "2026.07"' manifest.yaml
```

Use `treease operators list` and `treease operators get NAME` for the
installed operator summaries, syntax, examples, and limitations.

## Structured metadata

The command specification, operator catalog, and format catalog are exported
from the same source used by the CLI help output:

```bash
treease help --format json
treease operators list --format json
treease formats list --format json
```

This keeps generated documentation and tooling metadata aligned with the
actual parser and runtime capabilities.

## Error handling

Errors are written to standard error and the process exits with status `1`.
Use the command-specific help and discovery commands shown above when an
expression, format, operator, or input path is rejected.

## Project links

- Website: <https://treease.com>
- Source repository: <https://github.com/loggerhead/treease>
- CLI source: <https://github.com/loggerhead/treease/tree/main/apps/cli>
- Core crate: <https://crates.io/crates/treease-core>
- API documentation: <https://docs.rs/treease-cli>

## Local verification

```bash
cd apps/cli
cargo nextest run --locked --lib
bash tests/acceptance/run.sh
cargo doc --no-deps --lib
```
