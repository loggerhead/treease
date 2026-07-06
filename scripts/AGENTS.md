---
summary: "脚本与执行层治理入口，定义 docs 脚本与仓库级校验脚本的职责边界。"
read_when:
  - 需要修改 docs 相关脚本、索引生成器或校验器
  - 需要确认脚本应更新哪一层文档产物
---
# Scripts Guide

本目录承载仓库的脚本与执行层规则，尤其是文档索引、导航映射、校验链路与发布前检查。

## 适用范围

- `scripts/` 下所有仓库级脚本。
- 任何会生成、校验或发布 `docs/` 产物的命令。

## 职责边界

- `docs-list.mjs`
  - 负责输出当前文档树的机器可读导航清单。
- `generate-docs-map.mjs`
  - 负责生成 `docs/docs_map.md`。
- `check-docs.mjs`
  - 负责校验文档结构、入口承接、生成层边界与路径一致性。

## 规则

- 文档脚本必须服务当前文档层级，不得固化旧目录结构。
- 入口页规则以 `docs/AGENTS.md` 和 `docs/docs.json` 为准，脚本只做生成与校验，不另行发明一套导航真源。
- 脚本若依赖主题域入口命名，统一按 `index.md` 处理，而不是依赖 `README.md`。
- 生成产物只能落在约定路径，如 `docs/docs_map.md`、`docs/generated/*`。
- 修改脚本后，必须至少复核：
  - `pnpm docs:list`
  - `pnpm docs:map:check`
  - `node scripts/check-docs.mjs`
