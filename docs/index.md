---
summary: "Treease 文档首页，承接产品概览、主题导航、阅读顺序与 docs 站点总入口。"
read_when:
  - 需要从顶层进入 Treease 文档
  - 需要判断应先读哪个主题域入口
---
# Treease Docs

Treease 是一个面向结构化文本的 editor + Graph 工作台。它把文本编辑、结构理解、路径定位、图上浏览、局部编辑、格式转换和 CLI 工作流放到同一条主链里，而不是把这些能力拆成互相独立的小工具。

文档首页先回答三个问题：
- Treease 解决什么问题
- 应该从哪个主题域进入
- 修改实现时应该读哪条最短路径

## What Treease Covers

- 结构化文本导入、查看、编辑与导出
- Editor 和 Graph 之间的双向定位与双向编辑
- Runtime、snapshot、WASM 和 protocol 主链
- CLI 查询、转换、graph 页面与机器可读产物
- 针对 operators、formats、references 的手写说明与生成快照核验

## Start Here

- 初次了解产品目标与用户路径：`./start/index.md`
- 需要仓库级规则与实施边界：`../AGENTS.md`
- 需要 docs 元规则与层级约束：`./AGENTS.md`

## Theme Indexes

- [Start](./start/index.md)
  产品价值、用户路径与入门阅读顺序。
- [Web](./web/index.md)
  编辑器、GraphViewer、Worker、streaming、workspace 与前端消费链路。
- [Core](./core/index.md)
  协议真源、runtime、snapshot、WASM 与 Core 职责边界。
- [Testing](./testing/index.md)
  测试分层、验证命令、真实覆盖与跨边界验证原则。
- [CLI](./cli/index.md)
  命令入口、graph 页面、错误码和机器可读快照。
- [Operators](./operators/index.md)
  算子领域索引、正文入口与注册能力核验路径。
- [Formats](./formats/index.md)
  格式支持边界、独立正文与快照分工。
- [References](./references/index.md)
  语法、术语、边界文档与跨领域参考。
- [Generated](./generated/index.md)
  自动生成快照、索引产物与发布前核验层。

## Reading Paths

- Web 修改：`../apps/web/AGENTS.md` → `./web/index.md`
- Core 修改：`../packages/core/AGENTS.md` → `./core/index.md`
- CLI 修改：`../apps/cli/AGENTS.md` → `./cli/index.md`
- 测试决策：`./testing/index.md`
- 文档重构：`./AGENTS.md` → `./CODING.md`
- 需要看导航元数据：`./docs.json`
- 需要看标题映射产物：`./docs_map.md`

## Bump Versions

- 同步提升版本号入口：`pnpm bump:version -- --targets core,cli,web --part minor`
- `--targets` 支持任意组合，例如 `core`、`cli`、`web`。
- `--part` 支持 `patch`（默认）、`minor`、`major`。
- 当同时升级 `core` 与 `cli` 时，命令会检查 `apps/cli/Cargo.toml` 里的 `treease-core` 依赖是否可追踪到同一次 core 版本，并会同步写入。

## Verification

- `pnpm docs:list`
- `pnpm docs:map:gen`
- `pnpm docs:map:check`
- `node ./scripts/check-docs.mjs`
