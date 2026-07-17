# Treease

Treease 是一个面向结构化文本的工作台，用于检查、追踪、编辑、对比并导出 JSON、YAML、TOML、CSV 和嵌入式 payload。

## 为什么是 Treease

原始结构化文本往往在“难以编辑”之前，就已经先变得“难以理解”。Treease 把源文本和图形上下文绑定在同一份文档状态上，让你先看清结构，再追踪字段、检查改动、对比结果，并带着把握导出。

相比在普通编辑器、独立查看器和一次性转换命令之间来回切换，Treease 把文档本身、它的可视结构和下一步操作放进同一个工作流里。

## 核心能力

- 打开本地结构化文件，在不离开源文本的前提下看到它的真实结构。
- 在 graph、tree path 和源文本之间追踪字段而不丢失上下文。
- 在保持可视上下文的同时，对当前文档进行格式化、压缩、键排序和编辑。
- 在信任文本 diff 之前先做结构化对比，并在导出前预览转换结果。
- 通过 URL-backed preset 打开可复现的 editor / viewer 状态，便于分享示例、演示和问题复现入口。
- 通过 CLI 查询、转换并可视化结构化文档，包括只读的本地 web graph 视图。

## 快速开始

### Web

```bash
pnpm install
pnpm dev
```

### CLI

```bash
cargo install treease-cli

# 快速示例
treease '.services.api.url' example.json
treease -o yaml '.' example.json
treease web '.services.api' example.json

# 读取一个值
treease '.a.b' file.yaml

# 在格式之间转换
treease -p yaml -o json '.' file.yaml

# 直接写回文件
treease -i '.a = 1' file.yaml

# 查看 CLI 能力
treease help --format json
treease operators list
treease formats list --format json
treease examples search "filter array"
treease doctor --format json
```

## 资产配置说明

Web 应用和 `treease web` 可以从远端静态资源源站拉取资源。当前仓库里的默认值指向 Treease 自己管理的基础设施，主要用于官方部署路径，不应默认视为适合第三方直接复用的公共地址或 bucket。

- `PUBLIC_ASSET_BASE_URL` 用于控制 Web 应用的资源源站。
- `TREEASE_R2_ASSET_BUCKET` 用于 `apps/web/scripts/` 下的资源上传与检查脚本。
- CLI 构建也为 `treease web` 提供了一个默认的远端静态资源基地址。

如果你要做自托管或社区部署，应当为自己的环境显式配置这些值，而不是依赖仓库里的默认值。

## 开发

### 仓库结构

- `apps/web/`：Svelte Web 应用、编辑器 UI、图形 UI，以及浏览器 worker 边界。
- `apps/cli/`：独立 CLI crate、acceptance 测试和 CLI 元数据导出。
- `packages/core/`：Rust 解析器、格式化器、算子、求值、graph 构建、snapshot 和 WASM 导出。

### 常用命令

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

### 本地调试 `treease web`

当你需要在仓库里本地调试 CLI / Web 共用的 graph 页面时，先构建 Web 资源，再用仓库里的包装脚本指向本地 `cli-assets`，而不是公共资源地址：

```bash
pnpm --dir apps/web build
node ./scripts/treease-web-local.mjs . path/to/file.json
```

`node ./scripts/treease-web-local.mjs` 会为已构建的 CLI assets 目录启动本地静态服务器，用 `cargo run` 运行当前 checkout 的 CLI，并在 `.tmp/` 下创建隔离的 `TREEASE_WEB_CACHE_DIR`。这样即使 `wasm_release_date` 不变，本地 bundle 更新后也不会命中旧缓存。

如果你只想拿到手动运行所需的环境变量，可以执行：

```bash
node ./scripts/treease-web-local.mjs serve
```

它会打印本地 `TREEASE_WEB_ASSET_BASE_URL` 和配套的隔离 cache 路径，同时保持静态服务器以前台方式运行。

### 协议与 WASM 重新生成

当你修改 document protocol 或 Rust/WASM 边界时，执行：

```bash
cd packages/core
cargo run --locked --bin export_document_protocol

cd ../../apps/web
pnpm wasm:bindgen
pnpm wasm:sync
```

### 文档一致性检查

修改文档后执行：

```bash
node scripts/check-docs.mjs
```
