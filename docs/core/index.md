---
summary: "Core 主题域入口，承接协议真源、runtime、snapshot、WASM 与 Core 职责边界。"
read_when:
  - 任务涉及 packages/core、WASM、protocol、runtime 或 snapshot 语义
  - 需要判断能力应落在 Core 还是 Web
---
# Core

`core/` 承接 Treease 的计算层文档，解释哪些职责必须属于 Core，以及 Core 如何对外暴露协议、runtime 和 WASM 边界。

Treease 的正确性建立在 Core 对文档语义的单点定义上。只要一个能力的正确与否依赖 parser、operator、snapshot、graph build、protocol 或 runtime 事件语义，它就不该在 Web、Worker 或 CLI 各自解释一遍。

## Domain Rules

- 只要正确性依赖文档语义，它就应落在 Core。
- Web、Worker、CLI 不得各自复制 Core 的语义定义。
- protocol 真源在 Rust，生成物只是绑定层，不是第二套定义。

## What This Domain Covers

- parser、registry、operator、format 和 graph build 的语义所有权
- protocol、snapshot、Document Runtime 和 query/read 的边界
- WASM 绑定如何把 Core 结果暴露给 Web 和其他调用方
- 哪些变更必须先改真源，再刷新生成物

## Read By Topic

- `./wasm-language-packs.md`
  - WASM language pack 拆分与按需加载规划

## Stable Entry Points

- `packages/core/src/document/protocol.rs`
- `packages/core/src/wasm_document.rs`
- `packages/core/wasm/document-protocol.generated.ts`

## Relation To Other Domains

- 前端消费链路和交互落地：`../web/index.md`
- 测试分层与验证命令：`../testing/index.md`
