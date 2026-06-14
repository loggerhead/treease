# Treease Core 约束

## 适用范围
- 本文件只约束 `packages/core/`。
- 默认前置导航：`../packages/core/AGENTS.md`、`../packages/core/src/AGENTS.md`、`./agent-entrypoints.md`。

## Core 主链
- Document Runtime WASM 导出：`packages/core/src/wasm_document.rs`
- 兼容 / 非 document ABI：`packages/core/src/wasm.rs`
- 协议真源：`packages/core/src/document/protocol.rs`
- View 构建：`packages/core/src/core/graph_builder.rs`

## 必守规则
- Core 不直接处理 Web UI；浏览器侧能力必须通过 WASM 导出供 TS Worker 调用。
- 文档协议唯一来源是 `packages/core/src/document/protocol.rs`；生成物 `packages/core/wasm/document-protocol.generated.ts` 由 `cargo run --locked --bin export_document_protocol` 导出。
- `SnapshotReady { snapshotId, analysis, mainGraph }` 是主文档权威终态；`AnalysisDelta` 只表达流式过程中的临时可见结果。
- snapshot-bound read API 统一返回 `SnapshotReadResult<T>`；缺少 snapshot 时返回 `SnapshotNotReady`，不得偷偷创建 snapshot。
- hover 子图唯一入口是 `buildHoverSubgraphProjection({ snapshotId, path })`；主图只来自 `DocumentJob` 事件与 `SnapshotReady.mainGraph`。
- `packages/core/src/wasm_document.rs` 是 document runtime 边界；`packages/core/src/wasm.rs` 只保留兼容 / 非 document ABI，不是协议真源。
- same-language JSON TreeStore 契约固定：`Int` / `Float` 节点的 `value` 始终是合法 JSON number lexeme。
- Graph value edit 的格式感知局部替换规划归属 Core；Web 不复制 JSON/YAML 等格式语义。
- Web 主文档链路必须收敛到 `DocumentJob` / `DocumentSnapshot` 契约；不要演化出第二套 authority。
- wasm 下 tree-sitter allocator 必须在 `init_wasm()` 最早阶段安装；禁止恢复“每次调用 reset 独立 C bump allocator”的模型。

## 性能与变更流程
- `GraphBuilder.build` 维持 O(n) 级遍历，不引入多次全树扫描。
- 增量成功条件必须包含 `tree_path` / `path_span` 正确性；无法保证时必须 fallback。
- 本地 dev server 已启动时，完成 `pnpm wasm:sync` 后需要重启 dev server 或强制刷新 worker，避免旧 WASM 二进制配新 TS binding 导致 decode 错误。

