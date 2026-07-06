---
summary: "packages/core 的计算职责边界、协议约束与发布验证。"
read_when:
  - 需要确认 Core 变更是否影响 protocol、WASM 或语义边界
---

# packages/core 导航

## 作用域
- 本目录负责核心计算逻辑、WASM 导出、WASM TypeScript 适配、协议定义与 Core 测试。

## 硬约束
- 不在本目录实现 UI、Svelte、DOM 或浏览器交互逻辑。
- protocol 改动先改 `src/document/protocol.rs`，再导出生成物。
- `src/wasm_document.rs` 是 document runtime 导出边界；`src/wasm.rs` 只保留兼容 / 非 document ABI。
- 新逻辑优先进入既有模块，不新增并行实现。
- 修改手动内存、`Unmanaged`、Graph 增量或 legacy stream transport 时，补齐 `../../docs/CODING.md`。

## 测试约定
- 测试优先验证真实输入输出、错误路径与协议边界。
- 测试工具只留在 `tests/`，不放回生产源码。
- 测试命名延续当前模式：`operators_*.rs`、`core_*.rs`、`compare_*.rs`、`parser_*.rs`、`format*.rs`。

## 验证
- 默认 `cargo nextest run --locked`。
- 涉及 protocol 时，额外运行 `cargo run --locked --bin export_document_protocol`。
