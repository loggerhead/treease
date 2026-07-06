---
summary: "References 主题域入口，承接语义总览、边界文档与参考资料。"
read_when:
  - 需要先判断该看手写参考还是自动生成快照
  - 需要从 references 总览跳到具体参考文档
---
# References

`references/` 承接手写参考资料。这里负责解释能力分组、边界、术语和阅读顺序，不承担生成层职责。

当读者需要的不是某个具体算子的例子，而是“这整套语法/能力大致长什么样”“某类输入为什么支持到这里为止”“当前代码入口在哪”这类问题时，应先进入本域。

## Pages

- `supported-syntax-and-operators.md`
  - 语法与算子总览、能力分组、源码入口
- `yaml-common-subset.md`
  - 常见 YAML 子集、fixture 分类与 rare 数据集边界

## Related Layers

- 自动生成快照：`../generated/index.md`
- 算子正文入口：`../operators/index.md`
- 格式正文入口：`../formats/index.md`
