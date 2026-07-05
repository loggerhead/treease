---
summary: "Core 参考文档目录入口，说明手写参考页与生成快照的分工。"
read_when:
  - 需要先判断该看手写参考还是自动生成快照
  - 准备从 references 总览跳转到具体参考文档
---
# Core 参考资料

## 文档列表
- `supported-syntax-and-operators.md`：手写版语法与算子说明，面向阅读与设计核对
- `yaml-common-subset.md`：常见 YAML 子集、fixture 分类边界与 rare 数据集约束

## 说明
- 手写参考文档用于解释能力分组、边界、主要入口与阅读顺序
- 自动生成的能力快照放在 `../generated/`，用于核验当前构建的实际注册状态
