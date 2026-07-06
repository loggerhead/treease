---
summary: "Operators 主题域入口，承接算子领域索引、正文跳转与相关参考页。"
read_when:
  - 需要查找某个算子的正文入口
  - 需要在算子正文、参考总览和生成快照之间选择阅读顺序
---
# Operators

`operators/` 收编 Treease 算子正文。这里先说明这层解决什么问题，再把读者导向参考总览、生成快照和具体算子页。

这一层的职责不是列出“当前构建注册了哪些名字”，而是为那些需要额外语义说明、边界解释或例子的算子提供稳定正文。换句话说，`generated` 负责对账，`references` 负责总览，这里负责独立讲清楚单个算子。

## Read By Need

- 语法分组与能力总览：`../references/supported-syntax-and-operators.md`
- 当前构建注册状态：`../generated/core-registry-capabilities.md`
- 某个算子的独立正文：本目录下对应 `*.md`

## Scope

- 本目录承接已经拥有独立正文的算子页面。
- 不是所有已注册能力都会在这里拥有单独页面。
- 当某个算子需要完整示例、边界说明或设计语义时，正文应落在本目录，而不是散落到 references 或 generated。
