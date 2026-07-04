---
summary: "Document Runtime、snapshot、protocol 与领域术语的权威语境。"
read_when:
  - 任务直接涉及 Document Runtime、snapshot、protocol、mainGraph
  - 需要解释跨文档共享的核心术语
---
# CONTEXT.md

Treease 的全局领域语境与规范。agent 需要判断主文档链路、协议边界或术语含义时，以本文为准；执行计划、迁移步骤和临时 TODO 不放在这里。

---

## Document Runtime

Document Runtime 不是 `stream session` 协议整理，而是把 Treease 主文档链路收敛为 Rust 持有的 Document Runtime。用户依赖的是同一份文档在编辑、导入、分析、建图、定位与图上编辑中的一致状态，不是一组 transport 操作。

主链固定为：

```text
UI
  -> Worker transport / fan-out
  -> packages/core/wasm
  -> packages/core/src/wasm_document.rs
  -> packages/core/src/document/protocol.rs
  -> packages/core/src/document/runtime.rs
  -> packages/core/src 内部解析、增量、建图能力
```

核心约束：

- 顶层主语是 Document Runtime；streaming 只是输入和传输策略。
- `packages/core/src/document/protocol.rs` 是跨边界 document protocol 唯一真源。
- `cargo run --locked --bin export_document_protocol` 生成 `packages/core/wasm/document-protocol.generated.ts`。
- `packages/core/src/wasm_document.rs` 是 document runtime WASM 导出边界。
- `packages/core/src/wasm.rs` + `wasm/` 辅助模块只保留兼容期或非 document ABI，不承载主文档协议。
- 旧 `types.json`、`wasm_api_gen.ts`、公开 `bind_stored_document_snapshot`、`get_document_analysis`、`buildProjection('mainGraph')` 都不得重新成为主链入口。

## 领域模型

### Editor URL Preset

`Editor URL Preset` 是 `/editor` 路由在首次进入时由 URL 注入的一次性初始状态与动作集合。它只决定首屏展示、初始输入与可选的首轮动作执行，不代表后续由 URL 持续驱动运行时状态。

### Treease Expression

Treease Expression 是用户对结构化文档执行查询或变换的意图表达；它可以包含一个或多个 operator，但不等同于 operator 列表。需要描述整条求值逻辑时使用 `expression` / `表达式`，不要用 `operators` / `算子` 代称。

### Expression Result

Expression Result 是 Treease Expression 作用于输入 Document 后得到的派生结构化内容。它可以作为新的只读展示对象被解析和建图，但不代表原始 Document，也不默认拥有写回原始 Document 的语义。

### Document

有唯一 `document_key` 的文本内容实体。`document_key` 由上层分配，Rust runtime 按它管理 job、snapshot、freshness 与资源回收。

### DocumentSnapshot

Document 在某个时间点的不可变语义单元，由 runtime 分配 `snapshot_id`，按 `(document_key, snapshot_id)` 索引。它是 analysis、diagnostics、主图和可复用增量状态的唯一权威载体。

- `AnalyzeSource` / `ApplyEdits` 成功后提交 authoritative snapshot。
- parse failed 仍可提交 diagnostics-only snapshot。
- hover 子图是 transient projection，不推进文档权威状态。
- `snapshot_id` 不能由调用方预声明，不能跨 document 复用。

### DocumentJob

主文档状态推进只通过 `DocumentJob`：

- `AnalyzeSource`：打开、导入、全文粘贴、整文替换。
- `ApplyEdits`：文本编辑、图上编辑回写、批量 patch。

`ApplyEdits` 必须基于同一 document 的已提交 `base_snapshot`；缺少 base snapshot 时应 `Rejected`，Worker 不得捕获失败后改发 `AnalyzeSource`。增量路径不安全时，runtime 可以在同一个 `ApplyEdits` job 内基于 base snapshot + edits 做 full rebuild，对外仍是 `ApplyEdits` 结果。

Job API 形态：

```text
start_job(spec) -> JobHandle
advance_job(handle, input) -> EventBatch
cancel_job(handle) -> EventBatch
```

`AdvanceInput` 包含 `Poll`、`TextChunk`、`BinaryChunk`、`Close`；`TextChunk` 用于已在 Web 内存中的文本，`BinaryChunk` 用于 `ReadableStream<Uint8Array>` / file import 路径。`Close` 是输入结束的唯一显式信号。每个 job handle 一次性使用，只能产生一次 terminal，terminal 后不得再发业务事件。

### EventBatch 与事件

`EventBatch` 是每次推进 Job 返回的事件批次，携带 runtime request 身份、业务事件集合和可选 `JobTerminal`。`JobTerminal` 只表达生命周期终态，不承载 analysis 或 graph payload。

业务事件语义：

- `Progress { processedBytes }`：结构化运行进度；Web 不从日志字符串反推 byte offset。
- `AnalysisDelta`：流式过程中的临时可见 analysis；不提交 snapshot，不能作为 Query、Reveal 或 ApplyEdits 的权威基线。
- `SnapshotReady { snapshotId, analysis, mainGraph }`：提交 authoritative snapshot；`analysis` 只携带 diagnostics、semantic tokens、source size、language 等轻量元数据。
- `ParseFailed { snapshotId, analysis }`：提交 diagnostics-only snapshot；同批事件用主图 clear projection 清理可见 graph。

`DocumentAnalysisPayload` 供 `AnalysisDelta`、`SnapshotReady.analysis` 与 `ParseFailed.analysis` 共用，包含 `tree`、`valueJson`、`diagnostics`、`semanticTokens`、`sourceByteLength`、`language`，不携带 source 文本。主文档产品路径中 `AnalysisDelta`、`ParseFailed` 与成功 `SnapshotReady.analysis` 都不下发 full `tree` / full `valueJson`；结构和值读取必须通过同一 snapshot 的 projection query 获取。

### DocumentIntake

当前 Web 层的 intake 是 `apps/web/src/lib/services/DocumentIntake.ts` 里的薄编排函数 `runIntakeJob()`，只服务“文本已经在内存里”的 full-edit 场景（整文替换、语言切换、跨格式导入后的文本提交）。它当前只做三件事：

- 调用前执行一次 `isFresh` 检查。
- 调用 `runTextDocumentJobForGraph()`，由 `apps/web/src/lib/graph-stream/document-job-runner.ts` 完成 `start job → stream text → close → merge EventBatch`。
- 调用返回后再执行一次 `isFresh` 检查，并把 merged batch 收敛为 `IntakeResult { status, snapshotId, analysis, error? }` 交还 UI。

当前 intake 不拥有：
- chunk buffer、reader 生命周期或 stream / close 驱动；这些收敛在 `apps/web/src/lib/graph-stream/document-job-runner.ts` 与 `apps/web/src/lib/graph-stream/full-edit-document-job-session.ts`。
- Monaco 模型操作、`readOnly` 时机、可见文本 flush；这些在 `editor-full-edit-controller.ts`。
- freshness scope 创建；它只接受外部传入的 `isFresh` callback。

same-language 文件导入走另一条链路：`editor-full-edit-controller.ts` 直接 `File.stream().tee()`，一路给 UI flush 文本，一路调用 `startReadableDocumentJobSessionForGraph()`；session 内部通过 `runReadableDocumentJobForGraph()` 以 `BinaryChunk` 推进 readable job。这条 reader 路径不经过 `runIntakeJob()`，完成后把 readable job 的结果作为 `finishImportStream()` 的 `intakeOverride`。

`IntakeResult.status` 当前只有 `completed | failed`。只要 merged batch 提供了 `snapshotId`，即使该 snapshot 来自 `ParseFailed` 事件，intake 也会把它连同 diagnostics analysis 一起交还 UI；它不单独暴露 parse-failed 枚举。

### DocumentAnchor

当前 snapshot 上的定位语义，至少包含 path、span 与 target。editor reveal、graph click、breadcrumb、search、hover path 都必须基于 snapshot-bound anchor/query，不得各自维护定位规则。Path segment key 在协议边界上是 string；UI、Graph edit 和测试工具只消费 string。

## Snapshot-bound read APIs

只读 API 不推进文档状态，必须显式接收 `snapshotId`：

- `query_snapshot`：path/span lookup、anchor resolution、reveal query、root value kind、node preview、path scalar value、field labels。
- `buildHoverSubgraphProjection({ snapshotId, path })`：基于已有 snapshot 构建 transient hover 子图。

默认 UI 路径不得新增 full tree/full value query。hover、YQ completion、root scalar highlight、subgraph scalar content 与 graph edit 都应查询局部 projection；调试或导出若需要完整结构，必须另行定义 dev-only 能力，不能接入 `SnapshotReady` 主链。

统一返回：

```text
SnapshotReadResult<T> =
  { status: 'ready', data: T }
  | { status: 'snapshotNotReady' }
```

`SnapshotNotReady` 表示当前 Document 还没有可查询的 authoritative snapshot；它不是 parse failed，不是 transport error，也不是“查询结果为空”。读取 API 不得为了查询偷偷创建 snapshot，也不得用空结果混淆未就绪和无匹配。

## Projection 与主图

`ProjectionDelta` 是统一 graph 输出结构，包含 `clear` 与 `graph_data`；它不携带 kind，主图、hover 子图等语义由事件名、字段名或 API 上下文表达。

- 主图只能来自 DocumentJob streaming events 与 `SnapshotReady.mainGraph`。
- hover 子图只能来自 `buildHoverSubgraphProjection({ snapshotId, path })`。
- JSON 必须保留真实流式 decode、analysis、主图 delta、progress、parse-failed diagnostics-only。
- YAML / TOML / Python / JavaScript / CSV 对外复用同一 Job API；内部可 buffered 或 incremental。
- 非流式语言的 graph 在 Close 时通过 `SnapshotReady` 一次性送达。
- yq preview 的结果文本由 Core/WASM yq adapter 产出为 display-ready 文本；UI 不通过解析字符串反推 scalar/document 语义。

## Freshness 与 authority

Rust runtime 拥有 freshness、stale、authoritative/transient/diagnostics-only 归类、parse-failed materialize、graph clear、job 推进与 snapshot 提交语义。

Worker 只保留：

- transport
- request correlation
- UI fan-out
- UI 可见层 freshness guard

Worker 不拥有 session registry、graph chunk codec、parse-failed materialize、authority 二次裁决或 current snapshot 选择权。UI/主链侧持有 current snapshot，并在 reveal、path/span、hover 子图请求中显式传入 `snapshotId`。

## 输入策略

当前 public WASM job surface 只有一套 `startDocumentJob` / `advanceDocumentJob`：

- 无 edits 时，`start_document_job_impl()` 固定起 `AnalyzeSource + DocumentInputPlan::SourceText`；后续由 `TextChunk` / `Close` 推进。JSON 是否真流式，取决于 Rust 侧是否命中 streaming branch，而不是 start 时切到另一种 input plan。
- 有 edits 时，固定起 `ApplyEdits + DocumentInputPlan::BaseTextWithEdits`。
- reader-based 导入由 Web 层 `runReadableDocumentJobForGraph()` 直接发送 `BinaryChunk`，不在浏览器主线程做 UTF-8 decode；Core 在 close 时解码权威 source 文本，JSON streaming parser 在 chunk 推进时消费原始 bytes。
- 内存全文路径由 `runTextDocumentJobForGraph()` 把完整 string 切成 `TextChunk` 后走同一 API。
- `DocumentInputPlan::ByteStream` 仍存在于 Rust 内部类型里，但当前 Web/WASM 主链没有独立的 start-job 入口；它不是现行 document runtime 公共调用面。
- one-shot helper 只是把同一套 start / feed / close 包起来；主文档 full-edit、same-language 文件导入与 streaming graph 渲染仍走上述主链。

## 性能约束

- 一次解码，多路 fan-out。
- graph delta 优先于整图重传。
- 支持 incremental edit、局部 boundary rebuild 与 table cell patch。
- encoded payload 可以保留在 `ProjectionDelta` payload 内，但不得重新上浮为 channel / chunk framing 协议。

## 成立条件

Document Runtime 成立必须同时满足：

1. 顶层概念收口为 `DocumentSnapshot`、`DocumentAnchor`、`DocumentJob`、`EventBatch`、`JobTerminal` 与 snapshot-bound read APIs。
2. `document/protocol.rs` 是 document protocol 真源，`wasm_document.rs` 是 document runtime WASM 导出边界。
3. `wasm.rs`、旧 `types.json` 与旧 binding 体系不再被描述为主文档链路真源。
4. Worker 不再拥有 session registry、graph chunk codec、parse-failed materialize 或 authority 二次裁决。
5. 主图来自 streaming events 与 `SnapshotReady.mainGraph`；hover 子图来自 `buildHoverSubgraphProjection`。
6. JSON 仍具备真实流式 analysis + graph 输出。
7. diagnostics-only snapshot、graph clear、reveal、path/span 与 hover 子图都绑定同一份 snapshot 语义。
