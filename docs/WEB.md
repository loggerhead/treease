---
summary: "Web 多层架构、单向依赖与前端架构约束。"
read_when:
  - 任务涉及 apps/web、Worker、GraphViewer、store 或前端架构边界
  - 需要判断前端改动应落在哪一层
---
# Frontend 约束

本文只回答两个问题：

1. 前端多层架构如何分层
2. 前端各层之间必须遵守什么单向依赖约束

## 总体依赖方向

前端主链必须保持单向依赖：

```text
UI
  → Web 状态 / 服务编排
  → Web Worker
  → WASM 绑定
  → Core
```

禁止逆流：

- UI 不能直接访问 Core Rust 实现
- Web 状态层不能绕过 Worker 调 WASM 内部细节
- Worker 不能承担 Core 语义 authority

## 前端分层

### 1. UI 层

包括：

- routes
- Svelte 组件
- 编辑器与图形视图容器
- 可见交互、输入、展示

职责：

- 接住用户交互
- 绑定状态
- 渲染结果
- 维护局部视图生命周期

不负责：

- 文档语义计算
- 结构化读取真源
- 协议语义裁决

### 2. Web 状态 / 服务编排层

包括：

- store
- service
- controller
- 组件间共享状态编排

职责：

- 协调 UI 与 Worker
- 维护前端可见状态
- 组合业务场景
- 应用 freshness guard

不负责：

- 重新解析文档
- 重建 Core 应给出的 snapshot / graph / planner 语义

### 3. Worker 层

包括：

- `apps/web/src/workers/`
- runtime handler
- 请求 / 响应协议适配

职责：

- transport
- request correlation
- UI fan-out
- 错误序列化

不负责：

- authoritative / stale / diagnostics-only 语义判定
- 主图语义裁决
- snapshot identity 回退策略

### 4. WASM 绑定层

包括：

- `packages/core/wasm/index.ts`
- protocol 生成物
- TS 侧 WASM 适配

职责：

- 把 Core 暴露的协议和函数安全地接进 Web / Worker

不负责：

- 增加新的业务语义
- 代替 protocol 真源

## 前端内部架构约束

### UI 与状态

- UI 组件优先依赖 store / service / controller，不直接耦合远处组件内部实现
- 跨组件共享状态优先进入既有 store
- 局部渲染现场优先留在局部 runtime，不无故提升到全局 store

### GraphViewer

- `GraphViewer.svelte` 是稳定入口和组装层，不应继续膨胀成业务实现中心
- GraphViewer 具体能力应下沉到 `graph-viewer/` 子域
- Graph search、subgraph workspace、viewport、scene runtime 等都应有清晰子域

### Editor

- Editor 容器负责承接 Monaco 运行时与业务控制面
- 与文档语义直接相关的约束要通过数据流文档和双向编辑文档定义，不在 UI 层重新发明

### Workspace

- Workspace 是前端的工作区协调层
- 它可以保存可见状态和绑定关系
- 它不应重新成为文档语义 authority

## 复用与实现边界

- `apps/` 层应优先复用 Core 的既有链路，不得在应用层重复实现文档语义逻辑。涉及文档语义的能力（如解析、格式化、snapshot、graph、planner）必须在 Core 的 protocol/run-time 体系内闭环。

- 实现优先按语言无关逻辑落地；仅当存在明确、可量化收益时，才允许引入语言特异化逻辑，并且必须先获得你的书面同意。

## 单向依赖检查清单

- UI 是否直接读取了 Core 不该暴露的内部信息
- service / controller 是否绕过 Worker 直接依赖 WASM 细节
- Worker 是否在二次定义 snapshot / graph / stale 语义
- 某个状态是否本应是局部运行时状态，却被提升成全局状态
- 某个逻辑是否本应下沉到 Core，却在前端复制了一份格式或结构语义

## 设计目标

这套前端架构的目标不是“层数越多越好”，而是：

- UI 层只关心交互与呈现
- 前端状态层只关心编排与共享状态
- Worker 只关心跨边界 transport
- WASM 只关心绑定
- Core 才是文档语义真源
