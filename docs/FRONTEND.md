# Treease 前端约束

## 适用范围
- 本文件只约束 `apps/web/`。
- 默认前置导航：`../apps/web/AGENTS.md`、`../apps/web/src/AGENTS.md`、`./agent-entrypoints.md`。

## Web 主链
- `apps/web/src/lib/components/Editor.svelte`（核心在 `apps/web/src/lib/components/Editor/EditorCore.svelte`）→ `apps/web/src/workers/wasm-runtime.worker.ts` → `packages/core/wasm/index.ts` → `packages/core/src/wasm_document.rs` → `packages/core/src/document/`
- `GraphViewer.svelte` / `ViewportPanel.svelte` → Worker → `packages/core/wasm/index.ts` → `packages/core/src/wasm_document.rs` → `DocumentJob` 事件 / `SnapshotReady.mainGraph` / snapshot-bound 查询
- 协议链路：`packages/core/src/document/protocol.rs` → `cargo run --locked --bin export_document_protocol` → `packages/core/wasm/document-protocol.generated.ts`

## 目录职责
- `apps/web/src/lib/components/`：编辑器、图形视图与其他 UI 组件
- `apps/web/src/lib/components/graph-viewer/`：GraphViewer 领域实现
- `apps/web/src/lib/graph-stream/`：document job / active snapshot 主链上的统一图流执行域
- `apps/web/src/lib/import/`：导入源解析与导入配置；不承担 Core 编解码实现
- `apps/web/src/lib/graph/`：共享渲染模型、路径工具、视图状态与视口辅助
- `apps/web/src/shared/`：Worker / UI 共享的纯辅助函数，不承载状态或 authority
- `apps/web/src/workers/`：WASM Worker、请求协议、错误封装与运行时桥接
- `apps/web/test/`：集成与 E2E 测试辅助文件

## 必守规则
- Web 不得直接访问 `packages/core/src/`；所有核心能力必须走 `apps/web/src/workers` 与 `packages/core/wasm`。
- 文档协议唯一来源是 `packages/core/src/document/protocol.rs`；禁止手改 `packages/core/wasm/document-protocol.generated.ts`。
- Worker 统一返回 `ok/error`；跨边界错误必须序列化回传。
- 主文档 graph 只消费 `DocumentJob` 事件与 `SnapshotReady.mainGraph`；不要回到 `buildProjection('mainGraph')` 或其他 read API 补主图。
- snapshot-bound read API 必须显式带 `snapshotId`；缺少时返回 `SnapshotNotReady`，不得在读取 API 内偷偷建 snapshot。
- hover 子图唯一入口是 `buildHoverSubgraphProjection({ snapshotId, path })`。
- `getDiagnostics`、`parseToTree`、`parseValueToTree` 只做瞬时探测或非主文档缓存，不得回流成主文档 authority。
- JSON block 容错渲染只是光标派生 UI 状态；整文 invalid JSON 仍走 diagnostics-only 主链，不替代主文档 authoritative analysis。
- Monaco 语言高亮必须来自 Core/WASM 主链；禁止引入 `monaco-editor/esm/vs/basic-languages/*/*.contribution`，唯一例外是 settings 的 JSON 配置编辑。
- 跨组件共享状态优先走现有 store；不要直接耦合非父子组件。
- `apps/web/src/lib/components/ui/` 视为基础组件目录；新增通用组件前先确认 shadcn-svelte 是否已有现成实现。

## GraphViewer 边界
- `apps/web/src/lib/components/GraphViewer.svelte` 是稳定入口，只负责生命周期、controller 组装、test runtime 暴露与跨域编排。
- 主流程逻辑下沉到 `apps/web/src/lib/components/graph-viewer/`：scene runtime、render kernel、anchor、图文联动、hover panel、value edit、progress、dirty region、geometry、pointer、viewport、benchmark 与 probe。
- `apps/web/src/lib/components/graph-viewer/runtime/` 承载 table runtime 与虚拟窗口等子域基础设施。
- `apps/web/src/lib/leafer-minimap/`、`apps/web/src/lib/leafer-virtual-list/` 只处理 Leafer 相关渲染/滚动基础设施，不承载 graph 构建、tree 语义或 session 生命周期。

## Worker runtime 边界
- `apps/web/src/workers/wasm-runtime.worker.ts` 是稳定入口，只负责初始化、message dispatch、统一错误出口与 runtime 组装。
- `apps/web/src/workers/runtime/protocol.ts` 是 worker 请求 / 响应协议边界；新增能力先扩协议，再落 handler。
- `apps/web/src/workers/runtime/` 按领域拆分 parse、value edit、tree path、graph search、transform、compare、graph transport、state、logging 与 request utils。
- Worker 只负责 transport / correlation / UI fan-out；active snapshot、authoritative analysis 与 freshness owner 由 document runtime / snapshot contract 收敛。
- `document-value-edit.ts` 负责 `planGraphValueEdit`；格式感知局部 edit 规划必须由 Core/WASM 提供，Web 只转发 `DocumentTextEdit` 结果。

## 新鲜度、错误与性能
- 异步结果会写回 UI、store、editor model、graph scene、diagnostics 或 tree state 时，先建立 `FreshnessScope`。
- 新异步链路禁止手写多段 `token` / `revision` / `model` / `sessionId` 组合判断；统一使用 `freshness.step()` 或 `freshness.isCurrent()`。
- 增量更新以 `GraphDelta` 为准；无法保证 `treePath` / `pathSpan` / `reveal` 位置语义时，必须 fallback 到统一的 document job → snapshot → projection 主链。
- `fullEditUiState` 是 full-edit 可见层唯一控制面；GraphViewer、Editor 与进度浮层都基于它决定 attach / 只读 / overlay 收口。
- graph stream 与 build-graph progress / delta 事件统一使用 `streamRunId` 和 `eventSeq`；不要重新引入 `streamToken`、事件级 `seq` 或 `streamId`。
- `formatSourceOnClose` 规则：format / minify 写回、增量编辑、GraphViewer full-edit / incremental render、JSON block transient render 一律传 `false`；import、tab-reactivate、language-switch 才跟随用户 `formatting.smart`。
- Editor 多 tab 边界：`editorWorkspace.tabOrder` 只表示左侧用户 tab；`TabManager.svelte` 只持有 Monaco model 生命周期；当前激活左侧 tab 才能成为 `primary` 并镜像到 legacy `editorStore` 字段供 GraphViewer 消费。
- 右侧 editor 是固定 `sidecar` workspace tab，不出现在左侧 tab strip，不替换 GraphViewer authority；右侧语法高亮、semantic tokens 与 auto-format 必须继续走共享 Monaco runtime、Core/WASM formatting 和 `createWorkspaceTabFullEditSink`。
