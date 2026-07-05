---
summary: "Document Runtime、snapshot、protocol 与领域术语的权威语境。"
read_when:
  - 任务直接涉及 Document Runtime、snapshot、protocol、mainGraph
  - 需要解释跨文档共享的核心术语
---
# CONTEXT.md

本文是 Treease 主文档链路的领域语境真源。执行计划、临时 TODO、迁移步骤不放在这里。

## 顶层主语

Treease 的主文档链路顶层主语是 `Document Runtime`，不是“若干流式 helper 的组合”。

```text
UI
  → Worker transport / fan-out
  → packages/core/wasm
  → wasm_document.rs
  → document/protocol.rs
  → Document Runtime
```

streaming 只是输入和推进策略，不是顶层领域对象。

## 核心对象

### Document

带唯一 `document_key` 的文本实体。runtime 按它管理 job、snapshot、freshness、资源回收。

### DocumentSnapshot

某个 `document_key` 在某一时刻的不可变语义单元。

- runtime 分配 `snapshot_id`
- `SnapshotReady` 提交 authoritative snapshot
- blank / whitespace close 也提交 authoritative clear snapshot
- `ParseFailed` 提交 diagnostics-only snapshot
- hover / subgraph projection 不推进 authoritative state

### DocumentJob

主文档状态推进只通过 `DocumentJob`：

- `AnalyzeSource`
- `ApplyEdits`

`ApplyEdits` 必须基于同 document 的 `base_snapshot`；缺少时应拒绝，而不是偷偷改发 `AnalyzeSource`。

### EventBatch

每次推进 job 返回一个 `EventBatch`：

- `events`
- 可选 `terminal`
- `request_seq`

`terminal` 只表达生命周期，不承载 graph / analysis payload。

## 业务事件

- `Progress`
  结构化进度。
- `AnalysisDelta`
  流式过程中的临时可见 analysis；不是权威 snapshot 基线。
- `SnapshotReady`
  authoritative snapshot；主图以 `mainGraph` 收口。
- `ParseFailed`
  diagnostics-only snapshot；同批 clear graph。

## snapshot-bound read

只读 API 不推进状态，必须显式接收 `snapshotId`。

当前主文档链路允许的 snapshot-bound read 包括：

- anchor / reveal / path-span 相关 query
- `root value kind`
- `value preview`
- `path value`
- `field labels`
- `buildHoverSubgraphProjection({ snapshotId, path })`

统一返回：

```text
SnapshotReadResult<T> =
  { status: 'ready', data: T }
  | { status: 'snapshotNotReady' }
```

约束：

- 不允许在 read API 内偷偷创建 snapshot。
- 不允许用空结果伪装 `SnapshotNotReady`。
- 不允许按 `documentKey` 回退到 latest snapshot。

## 主图语义

- 主图只能来自 `DocumentJob` streaming events 与 `SnapshotReady.mainGraph`。
- hover / subgraph projection 不是主图来源。
- `SnapshotReady.mainGraph` 是 close 后的最终 authoritative graph。
- blank / whitespace close 必须以 `mainGraph.clear = true` 收敛。
- root scalar whole replace 必须清掉旧 root 图形残留。

## 输入策略

### AnalyzeSource

- 无 edits 时，统一起 `AnalyzeSource + SourceText`。
- JSON 是否真流式，取决于 Rust streaming branch，而不是另起另一套 public API。

### ApplyEdits

- 有 edits 时，统一起 `ApplyEdits + BaseTextWithEdits`。
- 允许在同一提交里命中 structural incremental、syntax incremental fallback 或 full rebuild fallback。

### readable stream

- same-language 文件导入通过 `BinaryChunk` 推进 readable job。
- Core 在 close 时解码权威 source 文本。

## authority 划分

Rust runtime 拥有：

- freshness / stale 判断
- authoritative / diagnostics-only 归类
- snapshot 提交
- parse-failed materialize
- clear graph
- snapshot-bound read

Worker 只保留：

- transport
- request correlation
- UI fan-out
- UI 可见层 freshness guard

## 成立条件

Treease 的主文档链路只有在以下条件同时成立时才算成立：

1. 顶层概念收口为 `DocumentJob`、`DocumentSnapshot`、`EventBatch`、snapshot-bound read。
2. `document/protocol.rs` 是协议真源。
3. `wasm_document.rs` 是 WASM 边界。
4. Worker 不再拥有 authority 二次裁决。
5. 主图来自 streaming events 与 `SnapshotReady.mainGraph`。
6. diagnostics-only、clear graph、subgraph projection、planner 都绑定同一份 snapshot 语义。
