# YAML 常见子集

本文档定义 Treease 在 fixture 与实现层优先保证的“常见 YAML 子集”。

## 目标

- 覆盖真实用户最常手写的 YAML 结构。
- 让 `test/fixtures/yaml/` 只承载常见子集的样例。
- 将规范级、高级特性和边角语法移到 `test/fixtures/yaml-rare/`，避免它们持续污染常规回归集。

## 常见 YAML 子集

当前常见子集包含：

- 单文档输入。
- 基础多文档分隔：`---`。
- block mapping：`key: value`。
- block sequence：`- item`。
- block mapping 与 block sequence 的常规嵌套。
- plain scalar、single quoted scalar、double quoted scalar。
- 常见 block scalar：`|`、`>`。
- 空文档与空流。
- 行内注释与独立注释行。

## 不属于常见子集

以下语法暂不作为常规 YAML 子集的一部分：

- `%YAML`、`%TAG`、reserved directives。
- 显式 tag：`!!str`、`!!int`、`!!map`、`!!seq`、`!!set`、`!!omap`、`!!binary` 等。
- 自定义 tag 与 tag shorthand。
- anchor / alias。
- 显式 mapping 语法：`? key` / `: value`。
- 依赖复杂 stream 规则的多文档组合。
- 以规范覆盖为目的的极端空 key、空 value、复杂 node property 组合。

## 当前失败集分类

当前 `test/fixtures/failure.txt` 中失败的 YAML fixture 按以下原则分类：

- 保留在 `test/fixtures/yaml/`：只保留仍属于常见子集、需要继续修复的样例。
- 迁移到 `test/fixtures/yaml-rare/`：凡是依赖上述高级特性的失败样例，统一视为罕见语法。

当前保留在常规 YAML 目录中的失败样例只有空流：

- `AVM7.1.yaml`
- `empty-stream.1.yaml`

其余当前失败的 YAML 样例均归为罕见语法样例，并迁移到 `test/fixtures/yaml-rare/`。
