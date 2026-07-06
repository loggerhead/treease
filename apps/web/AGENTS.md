---
summary: "apps/web 的层级边界、职责与验证约束。"
read_when:
  - 需要确认 web 改动是否触达正确层级
---

# apps/web 导航

## 作用域
- 本目录负责前端 UI、交互、Worker、静态资源与 Web 测试。

## 硬约束
- 不在本目录实现解析、算子、评估、格式化等 Core 计算逻辑。
- 所有 Core 能力调用必须通过 `src/workers` 与 `../../packages/core/wasm`。
- `src/lib/components/GraphViewer.svelte` 与 `src/workers/wasm-runtime.worker.ts` 保持薄壳。
- 协议字段改动只改 `../../packages/core/src/document/protocol.rs`；生成物不手改。

## 代码分包约定
- GraphViewer 逻辑优先进入 `src/lib/components/graph-viewer/`。
- Worker runtime 逻辑优先进入 `src/workers/runtime/`，不要回灌到 `src/workers/wasm-runtime.worker.ts`。
- 单元测试优先放相邻 `**/*.test.ts`。

## 验证
- 整体链路验证回到 `../../docs/testing/index.md`。
- 改 protocol / WASM 时，在 `../../packages/core/` 运行 `cargo run --locked --bin export_document_protocol`，再在本目录按需运行 `pnpm wasm:bindgen`。
