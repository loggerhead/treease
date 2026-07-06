---
summary: "Repository 的硬性约束与跨目录治理入口。"
read_when:
  - 需要确认任务边界、跨层职责与文档读取顺序时
---

# Repository Guidelines

## 0. 最高优先级：任何操作前先执行 `pnpm docs:list`
- 这是本仓库的硬性前置规则。
- 进入仓库后的第一条工作命令必须是 `pnpm docs:list`。
- 在执行 `pnpm docs:list` 之前，不要搜索、读业务文件、改代码、跑测试、执行其他命令，或基于仓库内容回答问题。
- 跑完 `pnpm docs:list` 后，先按输出中的 `Read when` 读取命中的文档；默认先读 `docs/index.md` 选择最短阅读路径，只有任务直接触及 Document Runtime、snapshot、protocol、mainGraph 语义时再补 `CONTEXT.md`。

## Project Overview
- Treease 是一个多格式结构化文档工具链：`packages/core/` 负责 Rust 解析/格式化/算子/评估/建图，`apps/web/` 负责编辑器与图形界面，`apps/server/` 负责账号 / 计费 / 分享 / AI server 能力，`apps/cli/` 负责独立 CLI crate、acceptance 测试与文档入口。
- 根文件只保留仓库级硬约束、跨层边界和稳定入口；文档层级、主题导航和内容组织下沉到 `docs/`。

## Routing
- docs 首页与主题入口：`docs/index.md`
- docs 元规则：`docs/AGENTS.md`
- scripts 层规则：`scripts/AGENTS.md`
- Web：`apps/web/AGENTS.md` → `docs/web/index.md`
- Core：`packages/core/AGENTS.md` → `docs/core/index.md`
- CLI：`apps/cli/AGENTS.md` → `docs/cli/index.md`
- 测试与验证：`docs/testing/index.md`

## Stable Entry Points
- Web 主链：`apps/web/src/lib/components/Editor.svelte` → `apps/web/src/workers/wasm-runtime.worker.ts` → `packages/core/wasm/index.ts` → `packages/core/src/wasm_document.rs` → `packages/core/src/document/*`
- Graph 链路：`apps/web/src/lib/components/GraphViewer.svelte` / `ViewportPanel.svelte` 消费 `DocumentJob` 事件、`SnapshotReady.mainGraph` 与 snapshot-bound 查询结果
- CLI 主链：`apps/cli/src/main.rs` → `apps/cli/src/lib.rs` → `treease-core`
- 协议真源：`packages/core/src/document/protocol.rs`；TypeScript 生成物是 `packages/core/wasm/document-protocol.generated.ts`

## Cross-Repo Rules
- 不要跨层绕行：Web 不直接引用 `packages/core/src`，Core 不承载 Svelte/DOM/浏览器逻辑，CLI 不复制 Core 实现。
- 稳定入口文件保持薄壳：`apps/web/src/lib/components/GraphViewer.svelte`、`apps/web/src/workers/wasm-runtime.worker.ts`、`packages/core/src/wasm_document.rs`。
- Web 只负责展示、交互、前端状态；解析、格式化、算子、评估、graph build 必须下沉到 `packages/core/`。
- 任何逻辑或 bug fix 都禁止通过 fallback、补丁式分支、静默降级、双写语义或“只修当前 case”的特判落地；必须直接修主链、协议真源或真实职责边界。
- Worker 新能力先改 `apps/web/src/workers/runtime/protocol.ts`，再落 handler；跨边界错误统一走 `ok/error`。
- snapshot-bound 读取必须显式带 `snapshotId`；不要在读取 API 内偷偷建 snapshot。
- Web 异步落地遵循现有 `FreshnessScope` / guard 语义；过期结果直接丢弃，不覆盖当前 UI 状态。
- 跨组件共享状态优先走现有 store；不要直接耦合非父子组件。
- 不手改生成文件：`packages/core/wasm/document-protocol.generated.ts`。
- 修改 `.rs` 文件后运行 `cargo fmt`。

## Commit Rules
- Git commit message 使用英文 Conventional Commits 规范，优先采用 `type(scope): summary`；版本升级使用 `chore(scope): bump ... to vX.Y.Z`。
- 需要触发 crate publish 的改动，提交时必须同时 bump 对应包的 version。
- 修改 `packages/core` 并需要发布 `treease-core` 时，更新 `packages/core/Cargo.toml` 的 version，并使用版本升级 commit（例如 `chore(core): bump treease-core to vX.Y.Z`）。
- 修改 `apps/cli` 并需要发布 `treease-cli` 时，更新 `apps/cli/Cargo.toml` 的 version，并使用版本升级 commit（例如 `chore(cli): bump treease-cli to vX.Y.Z`）。

## Verification
- 默认从 `docs/testing/index.md` 选择最小相关验证，不无差别全跑。
- Core 常用：`cd packages/core && cargo nextest run --locked`
- CLI 常用：`cd apps/cli && cargo nextest run --locked --lib`；`cd apps/cli && bash tests/acceptance/run.sh`
- Web 常用：`cd apps/web && pnpm test:unit` / `pnpm test:integration` / `pnpm test:e2e`
- Server 常用：`cd apps/server && ./node_modules/.bin/tsc -p tsconfig.json --noEmit`；`cd apps/server && node --import tsx --test src/**/*.test.ts`
- 文档变更：在仓库根目录运行 `node scripts/check-docs.mjs`
- 改协议或 WASM 后：`cd packages/core && cargo run --locked --bin export_document_protocol`，再在 `apps/web/` 运行 `pnpm wasm:bindgen`；必要时继续 `pnpm wasm:sync`

## Final Reminder
- 如果这次任务还没有执行 `pnpm docs:list`，立刻先执行它；在此之前，不要进行任何其他仓库相关操作。
