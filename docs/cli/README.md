# Treease CLI

Treease CLI keeps the execution path short:

```bash
treease '.a.b' file.yaml
```

Default stdout contains data output only. Discovery and diagnostics use explicit commands.

## Tasks

| Task | Command |
| --- | --- |
| Read a value | `treease '.a.b' file.yaml` |
| Convert YAML to JSON | `treease -p yaml -o json '.' file.yaml` |
| Open a readonly local graph page | `treease web '.services.api' config.yaml` |
| Write a file in place | `treease -i '.a = 1' file.yaml` |
| Inspect CLI schema | `treease help --format json` |
| List operators | `treease operators list` |
| Inspect one operator | `treease operators get select --format json` |
| List formats | `treease formats list --format json` |
| Find examples | `treease examples search "filter array"` |
| Check CLI capabilities | `treease doctor --format json` |

## Local Web Graph

`treease web [OPTIONS] <EXPRESSION> <FILE|->` evaluates one expression against one input source, freezes the encoded result, and serves a local readonly graph page.

```bash
treease web '.services.api' config.yaml
cat payload.json | treease web -p json -o json '.' -
```

The command prints a `127.0.0.1` URL containing a random token and keeps running in the foreground until interrupted. It does not auto-open a browser. The page can be refreshed while the command is still running.

The graph page uses the same Web `ViewportPanel` / `GraphViewer` stack as the editor, but it is fullscreen, graph-only, and readonly. Search, zoom, export, hover previews, and minimap behavior remain available when applicable.

Supported web options are format/display options: `--input-format`, `--output-format`, `--prettyPrint`, `--indent`, `--unwrapScalar`, and `--no-doc`. `treease web` rejects `--inplace`, `--exit-status`, multiple input files, and `--null-input`.

Expression evaluation failures are reported in the terminal and do not start a server. Graph parsing failures for the frozen result are shown in the Web page using the normal diagnostics view. Installed CLI builds must include embedded Web assets; development builds can point `TREEASE_WEB_DIST` at a built Web output directory.

## Machine-Readable Files

- `../generated/cli-help.json`
- `../generated/operators.json`
- `../generated/formats.json`

## Error Codes

CLI errors include stable codes in stderr text.

- `UNKNOWN_FLAG`
- `UNKNOWN_COMMAND`
- `MISSING_VALUE`
- `INVALID_FLAG_COMBINATION`
- `UNSUPPORTED_WEB_FLAG`
- `INVALID_WEB_INPUT_COUNT`
- `MISSING_WEB_ASSETS`
- `WEB_SERVER_ERROR`
- `WEB_FORBIDDEN`
- `UNSUPPORTED_FORMAT`
- `UNSUPPORTED_OPERATOR`
- `EXECUTION_ERROR`
- `IO_ERROR`
