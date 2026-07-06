---
summary: "按任务路由 agent 的最短阅读路径与稳定入口。"
read_when:
  - 需要快速判断当前任务先读哪篇文档
  - 需要在 Web、Core、CLI、测试之间选择最短入口
---
# Agent 最短路径

## 默认规则

- 本文默认在执行完 `pnpm docs:list` 后打开。
- 先按任务类型选“总语境 → 分层约束 → 专题链路”，不要先把所有文档读一遍。
- 先读局部 `AGENTS.md`，再回这里选主题文档。
- 搜索前先判断自己要找的是“结构语义”还是“文本字面量”；结构语义优先走项目提供的结构化索引工具，文本字面量再用搜索。

## 任务路由

| 任务类型 | 最短路径 |
| --- | --- |
| docs-only | 对应主题文档（按 `pnpm docs:list` 输出选择） |
| Web UI / store / Worker / GraphViewer | `../apps/web/AGENTS.md` → `./FRONTEND.md` |
| Server / auth / billing / share / AI / usage | `../apps/server/AGENTS.md` → `../CONTEXT.md` |
| Core Rust / WASM / protocol | `../packages/core/AGENTS.md` → `./CORE.md` |
| runtime / snapshot / protocol / mainGraph | `./CORE.md` 或 `./FRONTEND.md` → `../CONTEXT.md` |
| WASM language pack / YAML 按需加载 | `./CORE.md` → `./FRONTEND.md` → `./wasm-language-packs.md` |
| editor / workspace / sourceText / snapshot authority | `./editor-data-flow.md` |
| graph edit / planner / fallback / workspace content pane | `./bidirectional-edit-pipeline.md` |
| subgraph workspace / pane chain / content pane / workspace graph pane | `./FRONTEND.md` → `./subgraph-workspace.md` |
| JSON streaming / import / chunk / close | `./stream-pipeline.md` |
| layout / topology / dirty region / edge geometry | `./layout-pipeline.md` |
| 测试策略 / 验证命令 | `./TESTING.md` |

## 稳定入口

- Web 主链：
  `../apps/web/src/lib/components/Editor.svelte`
  → `../apps/web/src/workers/wasm-runtime.worker.ts`
  → `../packages/core/wasm/index.ts`
  → `../packages/core/src/wasm_document.rs`
- Graph 主链：
  `../apps/web/src/lib/components/GraphViewer.svelte`
  → `../apps/web/src/lib/graph-stream/`
  → `../packages/core/src/document/`
- 协议真源：
  `../packages/core/src/document/protocol.rs`
- CLI 入口：
  `../apps/cli/src/main.rs`

## 深读触发器

- 打开 `../CONTEXT.md`
  当任务直接涉及 `Document Runtime`、`DocumentSnapshot`、`DocumentJob`、`SnapshotReady`、`ParseFailed`、snapshot-bound read。
- 打开 `./FRONTEND.md`
  当任务涉及 `FreshnessScope`、GraphViewer/Worker、Workspace、subgraph workspace、Monaco/Leafer 交互边界。
- 打开 `./CORE.md`
  当任务涉及 protocol 真源、WASM 导出、query_snapshot、graph build、blank clear snapshot。
- 打开 `./TESTING.md`
  当任务要决定跑什么测试、补什么测试、断言什么结果。
