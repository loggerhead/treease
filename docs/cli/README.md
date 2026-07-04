---
summary: "Treease CLI 的常用命令、graph 页面与机器可读输出入口。"
read_when:
  - 任务涉及 CLI 行为、命令发现或 docs/generated 快照
  - 需要确认 treease web、本地 graph 页面或 CLI 错误码
---
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

The graph page uses the same Web `ViewportPanel` / `GraphViewer` stack as the editor, but it is fullscreen, graph-only, and readonly. Search, zoom, export, click reveal, and minimap behavior remain available when applicable.

Supported web options are format/display options: `--input-format`, `--output-format`, `--prettyPrint`, `--indent`, `--unwrapScalar`, and `--no-doc`. `treease web` rejects `--inplace`, `--exit-status`, multiple input files, and `--null-input`.

Expression evaluation failures are reported in the terminal and do not start a server. Graph parsing failures for the frozen result are shown in the Web page using the normal diagnostics view. On first launch, `treease web` downloads the matching Web asset bundle from the configured asset origin, caches it locally, and then serves the cached files on `127.0.0.1`. Subsequent launches reuse the local cache until the CLI version changes.

For local shared-page debugging inside this repository, prefer the repo wrapper instead of the public asset origin:

```bash
pnpm --dir apps/web build
node ./scripts/treease-web-local.mjs . path/to/file.json
```

The wrapper serves `apps/web/build/cli-assets` locally and assigns an isolated `TREEASE_WEB_CACHE_DIR` under `.tmp/`, which avoids stale bundle reuse when the local Web build changes without a new `wasm_release_date`.

## Machine-Readable Files

- `../generated/cli-help.json`
- `../generated/operators.json`
- `../generated/formats.json`
- 刷新命令：运行 `cd apps/cli && cargo run --locked --bin export_cli_metadata`

手写格式说明入口见 `../formats/README.md`；生成 JSON 快照反映当前构建支持的 CLI 格式与能力。

## Error Codes

CLI errors include stable codes in stderr text.

- `UNKNOWN_FLAG`
- `UNKNOWN_COMMAND`
- `MISSING_VALUE`
- `INVALID_FLAG_COMBINATION`
- `UNSUPPORTED_WEB_FLAG`
- `INVALID_WEB_INPUT_COUNT`
- `WEB_ASSET_DOWNLOAD_ERROR`
- `WEB_ASSET_MANIFEST_ERROR`
- `WEB_ASSET_CACHE_ERROR`
- `WEB_SERVER_ERROR`
- `WEB_FORBIDDEN`
- `UNSUPPORTED_FORMAT`
- `UNSUPPORTED_OPERATOR`
- `EXECUTION_ERROR`
- `IO_ERROR`
