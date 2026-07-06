---
summary: "Generated 主题域入口，承接自动生成快照、索引产物与校验层说明。"
read_when:
  - 需要确认 CLI/算子/格式能力清单是否与构建快照一致
  - 需要区分手写正文与自动生成产物的职责
---
# Generated

`generated/` 承接自动生成的校验产物。这里的文件服务于核验、对账和发布前检查，不承担新的真语义定义。

## Generated Assets

- `core-registry-capabilities.md`
- `operators.json`
- `formats.json`
- `cli-help.json`

## Rules

- 当手写正文与生成产物冲突时，以手写正文和实现真源为准。
- 生成产物用于回答“当前构建实际产出了什么”，而不是“产品应该怎么定义”。
- 修改能力导出链路后，应同步刷新 docs map 与相应快照。

## Related Domains

- CLI：`../cli/index.md`
- Operators：`../operators/index.md`
- Formats：`../formats/index.md`
- References：`../references/index.md`
