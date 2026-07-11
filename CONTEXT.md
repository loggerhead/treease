---
summary: "Document Runtime、snapshot、protocol 与领域术语的权威语境。"
read_when:
  - 任务直接涉及 Document Runtime、snapshot、protocol、mainGraph
  - 需要解释跨文档共享的核心术语
  - 任务涉及 Web Workspace 或 Desktop Workspace 的产品边界
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

## 产品工作区

### Web Workspace

通过浏览器提供的 Treease 核心工作区，用于编辑、理解、比较和转换结构化文本。它不是营销站点、教程、定价或公开分享页的统称。

### Desktop Workspace

通过桌面应用提供的 Treease 核心工作区，与 `Web Workspace` 共享同一套用户任务和交互语义，并可承载桌面环境特有的文件与系统集成。它不是 Treease 整个网站的桌面镜像。

### File-Linked Document

已关联一个本地文件的 `Document`。关联仅表示其默认保存目标，而不改变 `Editor Model` 作为当前草稿文本 authority 的地位。

### Workspace Tab

`Web Workspace` 或 `Desktop Workspace` 内承载一个主 `Document` 的独立工作单元。每个 Tab 保有自己的文本、语义绑定与本地文件关联；它不等同于 Compare 的辅助内容栏。

### Hot Exit

`Desktop Workspace` 对未保存 `Workspace Tab` 的本地保护与恢复语义。它保证应用正常退出或异常终止后都能恢复编辑现场，但不等同于把草稿静默写入关联文件。

### File Access Grant

用户明确交给 `Desktop Workspace` 的本地文件访问范围。它只覆盖具体文件的读取、保存与外部变更检测，不代表对其所在目录或用户文件系统的通用授权。

### Desktop Sign-in Return

用户在系统浏览器完成 `Authentication` 后回到 `Desktop Workspace` 的单次登录交接。它将已建立的 `User` 会话交回已运行的桌面工作区，而不代表在工作区内再托管一套登录页。

### Desktop Analytics

`Desktop Workspace` 为产品改进自动发送的最小化使用信号。它只可描述操作类型及结果，不得包含本地文件名、路径、文本、图语义、认证信息或本地唯一标识。

## Server-facing terms

### Treease Server

面向程序化调用方的公开 API 服务，用于承载账号、计费、分享与 AI 能力。它不承载 `apps/web` 或 `packages/core` 已实现的文档计算能力，也不是 `Document Runtime` 的同义词。

### API Endpoint

`Treease Server` 的公开网络入口。当前约定优先使用独立子域名 `api.treease.com`，而不是挂在产品站点子路由下。

### API Key

程序化调用方访问 `Treease Server` 时可使用的鉴权凭证。该能力不属于第一版范围；第一版不提供 API Key。

### Share Link

指向某个可公开访问资源的链接，用于把内容或结果对外分享。当前语境下的“分享”特指公开链接分享，不指协作授权或团队权限分配。第一版由 server 持久化分享内容并生成短链接，而不是把完整载荷直接编码进 URL。

### User

`Treease Server` 第一版的顶层主体。订阅状态、Share Link 与 AI 用量都归属于 `User`，当前不引入 `Workspace` 或 `Project` 作为独立主体。

### Authentication

`User` 身份建立与登录校验机制。当前约定使用 `Supabase Auth` 作为认证源，并支持 `Google`、`GitHub` OAuth 与邮箱 OTP 登录。
第一版 `Treease Server` 的受保护接口统一走 `User` 会话鉴权。

### Subscription

`User` 对 `Treease Server` 服务资格的计费状态。第一版订阅归属于 `User`，不归属于 `API Key`。

### Credits

`Treease Server` 用于度量 AI 能力消耗的产品层用量单位。它不是底层模型 token 的直接同义词，而是面向套餐、额度与展示的稳定计量单位。第一版默认在请求完成后按实际消耗结算，而不是在请求发起时预扣。

### Share Link Policy

`Share Link` 第一版不单独计费，但受 `Subscription` 套餐约束，例如数量上限、有效期或访问限制。默认有效期为 7 天；订阅用户可在套餐允许的上限内延长有效期。

### Shareable Resource

`Share Link` 指向的公开资源。当前已确认第一版包含 `Editor Text Snapshot` 与 `Command Run` 两类资源，不包含 AI 结果。

### Editor URL Command

`/editor` URL preset 中的内建动作参数，用于请求编辑器在打开后执行一个预定义动作。当前代码中的已知取值包括 `format`、`minify`、`sort`、`escape`、`unescape` 与 `compare`。

### Editor Text Snapshot

供 `Share Link` 公开访问的不可变编辑器文本快照。它不跟随源内容后续变化。

### Command Run

一次具体的 `Editor URL Command` 运行记录，包含关联的 `Editor Text Snapshot`、命令参数与执行结果。`Share Link` 分享的是这类运行结果，而不只是命令字符串本身。

### Share Page

面向人类访问者的 `Share Link` 展示页面。当前约定分享页运行在 `treease.com`，而不是 `api.treease.com`。

### AI Request

`Treease Server` 的一次单轮 AI 调用。第一版不建模多轮对话状态。

### AI Result

`AI Request` 的结构化输出结果。第一版 AI 能力只返回结构化结果，不以自然语言文本为主输出。

### AI / Share Boundary

AI 能力与分享能力相互独立。第一版 `Share Link` 不承载 AI 结果，第一版 AI 结果也不进入分享资源模型。

### Tree Path Set

某个文档可枚举 `Tree Path` 的集合，可作为 `AI Request` 的输入上下文之一。

### Suggest YQ

第一版 AI 主能力。它根据用户的自然语言 `instruction`，以及 `Editor Text Snapshot` 或 `Tree Path Set` 提供的上下文，生成结构化的 yq 表达式建议。

### Programmatic Caller

第一版 `Treease Server` 面向的程序化调用方。当前语境下它被收窄为已登录 `User` 驱动的程序化调用，而不是第三方公开 API 平台。

### Usage Ledger

记录 `Credits` 消耗与归属的用量账本。第一版与 `Subscription`、`Suggest YQ` 等高成本能力联动，用于实际消耗结算与额度判断。
