---
summary: "格式文档目录入口，说明手写格式页、CLI 能力快照与覆盖边界。"
read_when:
  - 需要先判断某个格式该看手写说明还是生成快照
  - 准备从格式总览跳转到具体格式文档
---
# 格式说明

## 入口
- 手写页面只覆盖当前需要补充边界、语义或示例的格式。
- CLI 当前支持矩阵与扩展名快照见 `../generated/formats.json`。
- 刷新 CLI 格式快照：运行 `cd apps/cli && cargo run --locked --bin export_cli_metadata`。

## 手写页面
- `csv.md` — CSV 解析、编码与表头约束
- `javascript.md` — JavaScript object literal 支持范围
- `python.md` — Python literal 支持范围
- `toml.md` — TOML table / array / dotted key 语义

## 说明
- JSON / YAML 目前没有单独格式页，默认以 CLI 能力快照、算子文档和测试为准。
- 这里的手写页面只覆盖目前需要额外解释的格式边界；它不是“所有支持格式都有一篇正文页”的承诺。
