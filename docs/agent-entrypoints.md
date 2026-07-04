---
summary: "按任务路由 agent 的最短阅读路径与稳定入口。"
read_when:
  - 需要快速判断当前任务先读哪篇文档
  - 需要在 Web、Core、CLI、测试之间选择最短入口
---
# Agent 最短路径

## 默认规则
- 本文默认是在执行完 `pnpm docs:list` 后打开；先按 `Read when` 判断是否命中，再沿最短路径继续读。
- 先用 CodeGraph / `search` / `find` 缩小范围，再用 `read` 读取必要行段。
- 默认不先读 `../CONTEXT.md`、`./user-stories.md`；只有任务直接触及对应语义时再补读。
- 先看局部 `AGENTS.md`，只在需要跨层边界时再回主题文档。

## 任务路由
- Web UI / Svelte / store / routes：`../apps/web/AGENTS.md` → `./FRONTEND.md`
- Web integration / E2E：`../apps/web/AGENTS.md` → `../apps/web/test/AGENTS.md` → `./TESTING.md`
- Core Rust 实现：`../packages/core/AGENTS.md` → `./CORE.md`
- Core tests：`../packages/core/AGENTS.md` → `./TESTING.md`
- CLI 行为 / acceptance：`../apps/cli/AGENTS.md` → `./TESTING.md`
- docs-only：`./README.md` → 对应主题文档
- protocol / runtime / snapshot / mainGraph：`./CORE.md` 或 `./FRONTEND.md`，再补 `../CONTEXT.md`
- graph stream / chunk / ProjectionDelta：补 `./stream-pipeline.md`
- graph layout / topology / changed-region relayout：补 `./layout-pipeline.md`
- editor ↔ graph / planner / incremental edit：补 `./bidirectional-edit-pipeline.md`

## 稳定入口
- Web 主链：`../apps/web/src/lib/components/Editor.svelte` → `../apps/web/src/workers/wasm-runtime.worker.ts` → `../packages/core/wasm/index.ts` → `../packages/core/src/wasm_document.rs`
- Graph 主链：`../apps/web/src/lib/components/GraphViewer.svelte` → `../apps/web/src/lib/graph-stream/`
- Core 协议真源：`../packages/core/src/document/protocol.rs`
- CLI 入口：`../apps/cli/src/main.rs`

## 深读触发器
- `../CONTEXT.md`：Document Runtime、DocumentJob、DocumentSnapshot、snapshot-bound read、subgraph workspace projection、ParseFailed、SnapshotReady
- `./FRONTEND.md`：FreshnessScope、GraphViewer/worker runtime 职责、Monaco/Leafer 交互边界
- `./CORE.md`：协议真源、WASM 边界、增量编辑约束、allocator / tree-sitter 规则
- `./TESTING.md`：最小相关验证、真实覆盖、timeout / mock / E2E 规则
