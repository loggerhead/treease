# Treease 文档索引

## 使用方式
- 人类读者：从 `../README.md` 进入，再按主题扩展阅读。
- Agent / coding worker：先读 `./agent-entrypoints.md`，按任务补 `./FRONTEND.md`、`./CORE.md`、`./TESTING.md` 或 `../CONTEXT.md`。
- 变更文档后运行 `node scripts/check-docs.mjs` 校验根 Markdown、`docs/` 与模块级 `AGENTS.md` 中的路径、链接和命令一致性。

## 文档列表
- `agent-entrypoints.md` — agent 最短路径与任务路由
- `CORE.md` — Core 规则、协议真源、WASM / runtime 约束
- `FRONTEND.md` — Web 规则、GraphViewer / worker / freshness 边界
- `TESTING.md` — 真实覆盖、验证命令、timeout / mock / E2E 规则
- `CODING.md` — 跨目录协作、可读性与所有权规则
- `cli-design.md` — CLI agent 友好设计调研
- `cli/README.md` — CLI 使用、发现命令与 agent 路由
- `formats/README.md` — 手写格式说明入口与生成格式快照说明
- `stream-pipeline.md` — 流式导入、真假流式、chunk/close、ProjectionDelta
- `layout-pipeline.md` — Graph node 生成、topology、布局与 changed-region relayout
- `bidirectional-edit-pipeline.md` — Editor ↔ Graph、snapshot-bound planner、增量与 fallback
- `web-graph-stream-benchmark.md` — Web graph stream 性能口径
- `user-stories.md` — 用户故事与价值对齐
- `operators/` — 算子正文文档，每文件一个算子
- `references/README.md` — Core 表达式与格式参考入口
- `generated/` — 自动生成的 CLI help、formats、operators 与 Core 能力快照
- `superpowers/plans/` — 执行计划
