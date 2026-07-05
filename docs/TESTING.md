---
summary: "测试分层、真实覆盖原则与最小验证策略。"
read_when:
  - 准备选择验证命令或补测试
  - 需要判断该写单测、集成测试还是 E2E
---
# Treease 测试约束

## 目标

- 覆盖真实用户依赖的主链，而不是覆盖率数字。
- 优先覆盖协议边界、authority 边界、失败路径、回归高发面。
- 让测试失败时能直接说明“哪条产品语义坏了”。

## 核心原则

- 真实覆盖优先于覆盖率数字。
- 行为验证优先于实现细节验证。
- 核心链路优先于表面分支。
- 失败信号必须可解释、可定位、可复现。

## 什么是伪覆盖

- 只执行文件加载或组件渲染，但不验证关键行为。
- 只断言 mock 被调用，而不验证最终输出、状态变化或协议结果。
- 只覆盖简单 happy path，却绕过失败路径、边界输入和回归风险点。
- 通过大量 stub、假数据或手写短路，让测试失去对真实链路的约束力。

## 什么是真实覆盖

- 覆盖用户真正依赖的能力：解析、格式化、视图构建、Worker/WASM 调用、关键 UI 交互。
- 验证输入到输出的主链，而不是只验证局部中间态。
- 断言对外可观察结果：事件序列、graph、diagnostics、错误响应、状态变化、最终文档文本。

## 分层策略

### Core

- 验证 parse / format / graph build / runtime / protocol / snapshot 语义。
- 优先写输入输出和事件序列测试，而不是 helper 调用测试。

### Web 单元

- 验证 store、controller、局部状态机、渲染辅助逻辑。
- 不要用过度 mock 去替代真实 Worker / WASM 协议行为。

### Web 集成

- 验证 Worker、WASM、protocol、graph stream、snapshot query、freshness 协作。
- 对 runtime 价值依赖真实链路的功能，这层比纯单元更重要。

### E2E

- 验证真实用户路径是否可完成。
- 数量少而关键，不负责承载所有边界分支。

## 如何选测试层级

- 纯算法、协议、事件序列、snapshot 语义：优先 Core 测试。
- store、controller、局部状态机、纯前端辅助逻辑：优先 Web 单元。
- 依赖真实 Worker/WASM 协作、协议序列化、跨模块收敛：优先 Web 集成。
- 依赖真实用户路径、浏览器交互、Monaco/Leafer 现场：优先 E2E。

一句话：
如果一个功能的价值依赖真实 runtime 链路，那就不要只写过度 mock 的单元测试。

## 断言规则

- 优先断言对外可观察结果：
  事件序列、`SnapshotReady.mainGraph`、`ParseFailed`、`SnapshotNotReady`、高亮 path、pane 状态、最终文档文本。
- 少断言内部 helper、调用次数、偶然顺序和脆弱样式。
- 快照只能做辅助，不能代替关键行为断言。

### 组织 case 的方式

- 先按“产品语义”分组，而不是按函数名分组。
- 每组至少覆盖：成功路径、失败路径、边界输入、回归风险点。
- 对同一功能的多个 fallback，不要只测最终成功；要测进入 fallback 的条件和 fallback 后的最终语义。
- 对增量链路，不要只测“有结果”；要测它是否错误退化成了另一条语义更重的链路。

### 断言优先级

- 第一优先级：最终文档文本、最终 graph、最终 snapshot、最终 diagnostics。
- 第二优先级：结构化中间结果，例如 `DocumentEvent` 序列、reveal path、pane 状态。
- 最后才是内部细节，例如 helper、调用顺序、局部缓存命中。

## streaming / graph / workspace 重点

- streaming：断言 close 前是否已有 `ProjectionDelta`，close 后是否以 final mainGraph 收口。
- clear snapshot：断言 blank / whitespace close 是否更新 authoritative snapshot 并清掉 graph。
- planner：断言 stale snapshot、wrong documentKey、unsupported edit 是否稳定 fallback。
- subgraph workspace：断言展开、pane 生命周期、dirty/focused 回灌保护、连续提交串行化。
- table reveal：断言离屏 cell 是否能先滚到可见区再高亮。

## timeout 与慢链路

- timeout 必须 `<= 5_000ms`。
- 超时先视为实现或时序问题，不靠单纯放宽 timeout 过测。
- 优先修真实慢链路：Worker/WASM、图渲染、导入、浏览器交互。

## Mock 与替身规则

- 只在边界外部依赖不可稳定接入时使用 mock。
- 对仓库内可真实调用的模块，优先走真实链路。
- 如果 mock 会掩盖协议错误、序列化错误、运行时错误或关键状态转换，则不应使用。

## UI 与 E2E 可测性

- UI 组件应提供稳定入口：`aria-label`、`role`、`title`、必要时 `data-testid`。
- E2E 选择器优先使用语义属性和 `data-testid`，不要长期依赖 class 层级、文本碎片或 DOM 顺序。
- Monaco、Leafer、虚拟列表这类复杂控件要提供测试钩子或稳定容器标识，避免测试绕过真实主链。

## Leafer / Graph E2E 约定

- 不做像素级 screenshot 回归，优先验证语义结果。
- 等待要对准业务完成信号，例如 editor ready、graph rendered、`expect.poll`，不要固定 sleep。
- 复杂画布交互优先通过测试 hook 或应用侧交互 API 驱动，断言结构化结果：
  bounds、world 坐标、hit path、treePath、highlight、lastReveal、sourceText。
- 子图 content pane 编辑回归优先断言：
  “连续输入后的最终 document/sourceText 语义”
  和
  “外部 refresh 不会回灌覆盖正在编辑的 Monaco model”。

## 文档运行时回归矩阵

最低证据应覆盖这些验收面：

- 模型与边界：协议真源、WASM façade、TS 生成物仍一致。
- 语义所有权：runtime 持有 authoritative / diagnostics-only / stale 语义；Worker 只做 transport / fan-out。
- JSON streaming + graph delta：chunk 前后和 close 收口语义稳定。
- blank / whitespace clear snapshot：空文档提交会更新 authoritative snapshot 并清掉 graph。
- 子图工作区：展开、pane 生命周期、content pane 编辑、连续提交不丢最新草稿。
- parse-failed / diagnostics-only：invalid 输入会清 graph 并保留 diagnostics。
- 双向编辑：Editor 改动和 Graph 改动都仍收敛到统一 snapshot 主链。

## 验证命令

- `packages/core`：`cargo nextest run --locked`
- `apps/cli`：`cd apps/cli && cargo nextest run --locked --lib`
- `apps/cli` acceptance：`cd apps/cli && bash tests/acceptance/run.sh`
- `apps/web` 单元：`pnpm test:unit`
- `apps/web` 集成：`pnpm test:integration`
- `apps/web` E2E：`pnpm test:e2e`
- `apps/web` 仓库级确认：`pnpm test`

## 执行约定

- 先跑最小相关验证，不默认全跑。
- 修 bug 时优先补能稳定复现问题的回归测试。
- 测试失败时，先判断测试是否代表真实产品语义；如果是，先修代码，不通过弱化断言过测。
- 对 `apps/web/src/**/*.test.ts` 的修改，优先运行 `pnpm test:unit`。
- 对 `apps/web/test/integration/**/*.test.ts` 的修改，优先运行 `pnpm test:integration`。
- 对 `apps/web/test/e2e/**/*.spec.ts` 的修改，优先运行 `pnpm test:e2e`。
- Monaco Editor 的文本输入在 E2E 中必须通过应用侧测试钩子完成，不得用键盘逐字输入或直接改 store 代替。
