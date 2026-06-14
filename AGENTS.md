# Repository Guidelines

## Project Overview
- Treease 是一个多格式结构化文档工具链：`packages/core/` 负责 Rust 解析/格式化/算子/评估/建图，`apps/web/` 负责编辑器与图形界面，`apps/cli/` 负责独立 CLI crate、acceptance 测试与文档入口。
- 先读 `docs/agent-entrypoints.md` 选择最短阅读路径；只有任务直接触及 Document Runtime、snapshot、protocol、mainGraph 语义时再补 `CONTEXT.md`。
- 根文件只保留跨仓约束与稳定入口；细节规则下沉到模块级 `AGENTS.md` 与 `docs/` 主题文档。

## Stable Entry Points
- Web 主链：`apps/web/src/lib/components/Editor.svelte` → `apps/web/src/workers/wasm-runtime.worker.ts` → `packages/core/wasm/index.ts` → `packages/core/src/wasm_document.rs` → `packages/core/src/document/*`
- Graph 链路：`apps/web/src/lib/components/GraphViewer.svelte` / `ViewportPanel.svelte` 消费 `DocumentJob` 事件、`SnapshotReady.mainGraph` 与 snapshot-bound 查询结果
- CLI 主链：`apps/cli/src/main.rs` → `apps/cli/src/lib.rs` → `treease-core`
- 协议真源：`packages/core/src/document/protocol.rs`；TypeScript 生成物是 `packages/core/wasm/document-protocol.generated.ts`

## Directory Map
- `apps/web/`：UI、前端状态、Worker、浏览器运行时
- `apps/web/src/workers/`：transport / correlation / fan-out / 统一错误出口
- `apps/web/test/`：集成测试与 Playwright E2E
- `packages/core/`：解析、格式化、算子、评估、snapshot authority、graph build、WASM 导出
- `packages/core/src/document/`：document runtime、job engine、snapshot、projection、protocol
- `packages/core/tests/`：Rust 集成、operator、corpus、graph、protocol 回归测试
- `docs/`：规则、参考、执行计划、产品与性能文档

## Cross-Repo Rules
- 不要跨层绕行：Web 不直接引用 `packages/core/src`，Core 不承载 Svelte/DOM/浏览器逻辑，CLI 不复制 Core 实现。
- 稳定入口文件保持薄壳：`apps/web/src/lib/components/GraphViewer.svelte`、`apps/web/src/workers/wasm-runtime.worker.ts`、`packages/core/src/wasm_document.rs`。
- Web 只负责展示、交互、前端状态；解析、格式化、算子、评估、graph build 必须下沉到 `packages/core/`。
- Worker 新能力先改 `apps/web/src/workers/runtime/protocol.ts`，再落 handler；跨边界错误统一走 `ok/error`。
- snapshot-bound 读取必须显式带 `snapshotId`；不要在读取 API 内偷偷建 snapshot。
- Web 异步落地遵循现有 `FreshnessScope` / guard 语义；过期结果直接丢弃，不覆盖当前 UI 状态。
- 跨组件共享状态优先走现有 store；不要直接耦合非父子组件。
- 不手改生成文件：`packages/core/wasm/document-protocol.generated.ts`。
- 不从上层“绕修” `deps/` vendor；只在明确任务下修改依赖目录。
- 修改 `.rs` 文件后运行 `cargo fmt`。

## Verification
- 默认从 `docs/TESTING.md` 选择最小相关验证，不无差别全跑。
- Core 常用：`cd packages/core && cargo nextest run --locked`
- CLI 常用：`cd apps/cli && cargo nextest run --locked --lib`；`cd apps/cli && bash tests/acceptance/run.sh`
- Web 常用：`cd apps/web && pnpm test:unit` / `pnpm test:integration` / `pnpm test:e2e`
- 文档变更：在仓库根目录运行 `node scripts/check-docs.mjs`
- 改协议或 WASM 后：`cd packages/core && cargo run --locked --bin export_document_protocol`，再在 `apps/web/` 运行 `pnpm wasm:bindgen`；必要时继续 `pnpm wasm:sync`

## Key References
- `docs/agent-entrypoints.md`：按任务路由的最短路径
- `docs/FRONTEND.md`：Web 规则、职责边界、freshness / graph / worker 约束
- `docs/CORE.md`：Core 规则、协议边界、WASM / runtime 约束
- `docs/TESTING.md`：真实覆盖、timeout、mock 与 E2E 规则
- `scripts/check-docs.mjs`：文档路径、命令、选择器一致性校验
- `.github/workflows/core.yml`、`.github/workflows/web.yml`：CI 入口
