---
summary: "Formats 主题域入口，承接格式说明、支持边界与 CLI 快照的阅读路径。"
read_when:
  - 需要判断某个格式该看手写说明还是生成快照
  - 需要从格式总览跳到具体格式页面
---
# Formats

`formats/` 解释 Treease 的格式支持边界。这里先说明 Treease 对格式文档的写法，再告诉读者哪些格式有手写正文，哪些格式应以快照和测试为准。

格式页不是为了复制一份 CLI 支持表，而是为了补充那些仅靠快照名称无法说明清楚的边界：如何 parse、如何 encode/decode、Web 如何导入导出、哪些容器形态可 roundtrip、哪些能力只是交换格式而不是 editor language。

## Read By Need

- 当前 CLI 支持矩阵与扩展名快照：`../generated/formats.json`
- 当前有独立正文的格式：
  - `csv.md`
  - `javascript.md`
  - `python.md`
  - `toml.md`

## Scope

- 手写页面只覆盖需要额外说明边界、语义或示例的格式。
- JSON / YAML 没有独立格式页时，以 CLI 快照、算子文档、实现和测试为准。
- 本目录不是“所有支持格式都必须有一篇正文”的承诺。
