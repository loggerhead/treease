---
summary: "CLI 主题域入口，承接常用命令、graph 页面、机器可读产物与错误码。"
read_when:
  - 任务涉及 CLI 行为、命令发现或 docs/generated 快照
  - 需要确认 `treease web`、本地 graph 页面或 CLI 错误码
---
# CLI

`cli/` 承接 Treease 命令行的对外入口。这里先回答 CLI 能做什么，再把读者导向生成快照、格式说明和运算符索引。

## Common Tasks

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

## Local Graph Page

`treease web [OPTIONS] <EXPRESSION> <FILE|->` 对单个输入源执行表达式求值，冻结结果，并启动本地只读 graph 页面。

- 页面复用 Web 侧 `ViewportPanel` / `GraphViewer` 栈。
- 首次运行会下载并缓存对应版本的 graph 资源。
- 本地仓库调试时，优先使用 `node ./scripts/treease-web-local.mjs`，避免依赖公共资源源站。

## Machine-Readable Outputs

- `../generated/cli-help.json`
- `../generated/operators.json`
- `../generated/formats.json`
- 刷新命令：`cd apps/cli && cargo run --locked --bin export_cli_metadata`

相关入口：

- 格式说明：`../formats/index.md`
- 算子索引：`../operators/index.md`
- 生成层说明：`../generated/index.md`

## Error Codes

CLI 错误在 stderr 中带有稳定代码：

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
