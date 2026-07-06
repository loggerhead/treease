---
summary: "Core 职责定义与对外边界约束。"
read_when:
  - 任务涉及 packages/core、WASM 导出、协议真源或 Core 职责边界
  - 需要判断某个能力是否必须下沉到 Core
---
# Core 约束

本文只回答两个问题：

1. 哪些职责必须属于 `packages/core/`
2. Core 对 Web / Worker / WASM 的对外边界是什么

## Core 的职责

Core 是 Treease 的计算层，负责所有与“文档语义”直接相关的能力。

### 必须放在 Core 的能力

- 文档解析与解码
- 文档格式化与编码
- 算子、表达式求值与结构变换
- 文档运行时：job、snapshot、freshness、stale、diagnostics-only
- graph build、projection、layout 所依赖的结构语义
- graph value edit 的格式感知规划
- snapshot-bound read
- protocol 真源

### 不应放在 Core 的能力

- Svelte / DOM / 浏览器交互
- Monaco / Leafer 的运行时状态
- 前端路由、页面编排、组件生命周期
- Worker 的 transport / correlation / fan-out
- 任何“为了迁就 UI 现状”而复制的一份平行语义

一句话：

```text
只要能力的正确性取决于文档语义本身，它就应该在 Core；
只要能力的正确性取决于浏览器交互现场，它就不应该在 Core。
```

## 对外边界

### 稳定入口

- 协议真源：`packages/core/src/document/protocol.rs`
- 主文档 WASM 边界：`packages/core/src/wasm_document.rs`
- compat / 非 document ABI：`packages/core/src/wasm.rs`

### 协议边界

- document protocol 的唯一真源是 `document/protocol.rs`
- `packages/core/wasm/document-protocol.generated.ts` 只是生成物，不能手改
- Web 侧新增能力时，先改 protocol 真源，再导出生成物，再接 Worker / UI

### runtime 边界

- Core 负责 authoritative / diagnostics-only / stale 的判定
- Core 负责 `SnapshotReady`、`ParseFailed`、`ProjectionDelta` 的语义
- Core 负责 snapshot-bound read 的真实结果
- Web / Worker 不能二次裁决这些语义

### graph edit 边界

- graph value edit 的格式语义、fallback reason、局部替换规划都归 Core
- Web 只消费规划结果，不复制 JSON / YAML / CSV 规则

## 主图与读取边界

- 主图来自 `DocumentJob` 事件与 `SnapshotReady.mainGraph`
- snapshot-bound read 必须显式带 `snapshotId`
- `query_snapshot` 只读取调用方请求的那个 snapshot，不回退到 latest snapshot
- `buildHoverSubgraphProjection({ snapshotId, path })` 是 hover / subgraph 的 Core 投影入口

## 设计约束

- 不新增第二套主文档 authority
- 不让 Web 通过临时 parse 或缓存重建 Core 应提供的结构语义
- 不为了某个前端场景把一次性逻辑硬编码到 protocol 或 snapshot 契约里
- 不把生成物、compat 接口、测试 helper 当成协议真源

## 生产环境变更边界

- Core 新引入任何生产环境依赖前，必须先征得你的明确同意（文字化确认）。  
  包括但不限于新增/升级外部 crate、运行时绑定依赖、以及会影响交付环境的可执行链路变更。

- 所有实现均按语言无关逻辑优先。除非有明确收益且必须进行的场景，否则不应做语言特异化实现；语言特异化必须先获你明确同意。

## 常见判断题

### “这个能力应不应该下沉到 Core？”

如果它满足下面任一条件，通常就应该下沉：

- 需要理解文档的结构语义
- 需要保证跨格式一致性
- 需要绑定 snapshot identity
- 需要给 Graph / Editor / Search / Planner 提供一致结果

### “这个能力能不能留在 Web？”

如果它满足下面全部条件，通常可以留在 Web：

- 只影响可见交互和渲染现场
- 不重新定义文档语义
- 不需要成为跨组件、跨会话的权威状态
- 不要求与 Core 的 snapshot / graph / planner 结果完全一致

## 变更流程

- 改 `.rs` 文件后运行 `cargo fmt`
- 改 protocol 或 WASM 导出后：
  1. `cd packages/core && cargo run --locked --bin export_document_protocol`
  2. 按需在 `apps/web/` 执行 `pnpm wasm:bindgen` / `pnpm wasm:sync`
