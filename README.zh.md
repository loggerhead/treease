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

## Web CLI 配置

`treease web` 打开托管的 `/editor` 页面，并由 CLI 在 localhost 上短暂提供输入数据。自托管 Web 时，可用 `TREEASE_WEB_URL` 指定部署的 `/editor` 地址。

## 仓库边界

本仓库包含公开的 Web、Desktop、CLI、Core 和协议包。
账号、计费、分享、AI、用量和文件存储等 Hosted API 实现维护在独立的私有仓库中，公开客户端仅通过文档化的 HTTPS API 边界访问。

## 许可证

Treease 使用 [Treease Community License](LICENSE) source-available 许可证发布，
不是 OSI 认可的开源许可证。将 Treease 用于商业产品或托管服务前，请先阅读完整许可证条款。

## 参与贡献

开发检查、边界约束和 Pull Request 要求见 [CONTRIBUTING.md](CONTRIBUTING.md)。

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
pnpm test:full
pnpm test:e2e:coverage

cd ../..
pnpm test:coverage:core  # 需要先安装 cargo-llvm-cov
cd packages/core
cargo nextest run --locked

cd ../../apps/cli
cargo nextest run --locked --lib
bash tests/acceptance/run.sh
```

### 本地调试 `treease web`

先启动 Web 开发服务，再把 `TREEASE_WEB_URL` 指向本地 `/editor`：

```bash
pnpm --dir apps/web dev --host localhost.treease.com --port 5173
TREEASE_WEB_URL=http://localhost.treease.com:5173/editor \
  cargo run --manifest-path apps/cli/Cargo.toml -- web '.' path/to/file.json
```

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
