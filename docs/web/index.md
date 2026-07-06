---
summary: "Web 主题域入口，承接前端架构、编辑器、GraphViewer、Worker 与相关专题页。"
read_when:
  - 任务涉及 apps/web、编辑器、图视图、Worker 或前端运行时边界
  - 需要在 Web 相关专题页之间选择最短阅读路径
---
# Web

`web/` 收编 Treease 前端相关文档：架构边界、主文档数据流、双向编辑、streaming、layout、workspace 与性能专题。

Treease Web 的核心职责不是重新定义文档语义，而是消费 Core 已经确定的语义结果，并把这些结果稳定地落到 editor、graph、workspace 和异步交互里。这个域首先回答前端该拥有什么 authority、不该拥有什么 authority，以及数据如何沿主链流动。

## Domain Rules

- Web 只负责交互、展示、前端状态与浏览器运行时。
- 文档语义、snapshot、graph build、planner 语义仍以 Core 为真源。
- 任何前端专题都应能回收到本页，而不是在根目录孤立悬挂。

## What This Domain Covers

- Editor / Graph / Worker 之间的消费链路
- 主文档文本 authority、snapshot 绑定和局部视图行为
- graph edit、subgraph workspace、streaming 和 layout 这些前端专题如何挂回主链
- Web 侧性能和可见反馈如何建立在 runtime 事件之上

## Read By Topic

- 主文档主链与 authority：`./editor-data-flow.md`
- 图文双向编辑与 planner / fallback 边界：`./bidirectional-edit-pipeline.md`
- subgraph workspace 生命周期：`./subgraph-workspace.md`
- JSON streaming 与 close 收尾：`./stream-pipeline.md`
- graph layout、topology 和 dirty region：`./layout-pipeline.md`
- graph stream benchmark 基线：`./graph-stream-benchmark.md`

## Relation To Other Domains

- protocol、snapshot、WASM 和 runtime 真源：`../core/index.md`
- 验证命令、测试层级和真实覆盖原则：`../testing/index.md`
