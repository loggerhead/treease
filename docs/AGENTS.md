---
summary: "Treease 文档子树治理规则与导航契约。"
read_when:
  - 需要统一 docs 分层、生成文件、导航口径或新建主题文档时
  - 需要确认文档引用是否应走导航页而非逐页堆砌
---
# Docs Guide

本目录承载文档治理层规则，规定 docs 站点的层级、导航、生成物边界与入口承接关系。

## 层级

1. 首页与索引层
   - `docs/index.md`
   - `docs/docs_map.md`
2. 主题域入口层
   - `docs/start/index.md`
   - `docs/web/index.md`
   - `docs/core/index.md`
   - `docs/testing/index.md`
   - `docs/cli/index.md`
   - `docs/operators/index.md`
   - `docs/formats/index.md`
   - `docs/references/index.md`
   - `docs/generated/index.md`
3. 具体内容层
   - 各主题域目录下的正文页面
4. 生成与校验层
   - `docs/generated/*`
   - `docs/docs_map.md`
   - `docs/docs.json`

## 元规则

- 每个手写 Markdown 页面都必须包含 `summary` 和 `read_when` frontmatter。
- 入口页负责导航、阅读顺序和边界说明；正文页负责具体知识内容，不重复承担总入口职责。
- 任何正文都必须被某个主题域入口页收编。
- 任何自动生成产物都必须在 `docs/generated/index.md` 或 `docs/docs_map.md` 中被声明为生成层资源。
- `docs/docs.json` 是 docs 站点结构元数据入口；脚本应消费它，而不是旁路定义另一套结构。
- 手写 Markdown、示例命令、链接目标与生成说明都不得写入本机身份信息；统一使用仓库相对路径、主题页相对路径或脱敏占位符。

## 路径约定

- 主题域入口统一命名为 `index.md`。
- `docs/` 根目录只保留治理层、首页与索引层、跨域规则页。
- 专题正文优先下沉到所属主题域目录，不在 `docs/` 根目录长期悬挂。

## 生成层边界

- `docs/docs_map.md` 由 `pnpm docs:map:gen` 生成，不手工维护。
- `docs/generated/*.json`、`docs/generated/*.md` 是快照和校验产物，不承载新的真语义定义。
- 当手写正文与生成产物冲突时，以手写正文和实现真源为准，生成产物用于核验。

## 验证

- `pnpm docs:list`
- `pnpm docs:map:gen`
- `pnpm docs:map:check`
- `node scripts/check-docs.mjs`
