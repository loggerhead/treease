---
summary: "算子正文文档目录的阅读入口与目录分工说明。"
read_when:
  - 需要了解算子文档目录的组织方式
  - 准备从总览跳转到具体算子文档
---
# 算子文档

## 目录分工
- `*.md`：当前已经补了正文的算子页面，包含语义、完整示例与使用说明；不是所有已注册能力都已经有独立正文页。
- `../references/supported-syntax-and-operators.md`：手写总览，解释语法分组与源码入口。
- `../generated/core-registry-capabilities.md`：自动生成快照，精确反映当前构建注册了哪些内部能力符号。

## 阅读顺序
- 想看“当前支持了哪些能力”：先看 `../references/supported-syntax-and-operators.md`。
- 想确认“当前构建到底注册了哪些能力”：再看 `../generated/core-registry-capabilities.md`。
- 想看某个能力是否已有完整正文示例：最后再回本目录查对应 `*.md`。
