# Treease 前端约束

## 适用范围
- 本文件只约束 `apps/web/`。
- 默认前置导航：`../apps/web/AGENTS.md`、`./agent-entrypoints.md`。

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
- Graph hover 不再承担任何预览职责；局部阅读与展开统一改为 click 打开底部工作区。
- `SnapshotReady.sourceText` 是 editor/store 的 authoritative 写回文本；`parseFailed` 不提供替换用 `sourceText`。
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
- 子图工作区是 GraphViewer 底部的持久工作区，不是 hover panel 的样式变体。主图与工作区内的 graph pane 都通过 click 直接打开下一层 pane，不再依赖 hover 生命周期。
- 子图工作区唯一 authority 仍是 snapshot-bound `buildHoverSubgraphProjection({ snapshotId, path })`。主图或子图 pane 的点击只负责提供 `path` 与交互意图；workspace 不得自己构建主图，也不得绕过 active snapshot 读临时 projection。
- 子图工作区的渲染逻辑必须复用 graph 组件语义：每个 pane 是一个独立 DOM host，内部挂同样的 Leafer canvas / node / edge 渲染链路，并保留与主图一致的默认缩放、拖动和平移边界约束。
- 子图工作区链路遵循“最多保留 3 个 pane”的列栈规则。打开某个 pane 内的新子图时，只保留它的祖先链和新子图，丢弃右侧旧分支；不维护无限深历史。
- 子图工作区 path 轨默认不单独展示为 UI 轨道；pane 标题沿用 graph meta path 的截断口径，完整 path 只放在 header `title` / 调试信息里。
- 子图工作区点击 cell 时，业务语义分两段：一是继续走现有 reveal / editor 联动；二是在同一次点击里展开或替换底部 pane。`value` 打开对应 path，`key` 打开对应 key-value pair，`index/row` 打开对应 row；object/array 落 graph pane，scalar 落 content pane。主图与子图统一使用现有 cell 黄色高亮，不为 workspace 额外引入第二套高亮体系。
- 子图工作区编辑逻辑复用当前 graph cell 编辑链路；workspace 自己不实现独立的 value edit authority。是否可编辑、如何提交、提交后如何回流，仍由现有 GraphViewer value-edit controller 和 Core/WASM edit plan 决定。
- content pane 使用 Monaco hidden workspace tab 承载编辑器运行时，复用与右侧 sidecar editor 相同的 Monaco/runtime/full-edit 基础设施，但业务提交仍必须回到 `applyGraphEdit -> planGraphValueEdit -> Editor ApplyEdits` 主链，不能把 hidden tab 当成新的文档 authority。
- content pane 不再单独展示 key 输入框；path 继续沿用子图 pane 的 header 风格展示，局部编辑默认只对 value 生效。
- 子图工作区 graph cache 只缓存同一 documentKey / snapshotId / renderConfig 组合下的 projection 结果。renderConfig、revision、snapshot 或 enableNest 变化时必须整体失效并重建 pane 内容。
- 子图工作区允许对平移做边界限制，但不要限制缩放、点击命中或编辑行为。限制口径是 graph world bounds 外扩 500px，主图和 workspace pane 统一使用同一套几何规则。

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
