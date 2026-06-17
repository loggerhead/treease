# Treease 测试约束

## 目标
- 测试必须提供真实覆盖，而不是制造“看起来有覆盖”的伪覆盖
- 测试优先覆盖核心链路、协议边界与高风险行为，而不是低价值表面代码
- 测试结果要能帮助 agent 和开发者判断系统是否真的可用

## 核心原则
- 真实覆盖优先于覆盖率数字
- 核心链路优先于边缘路径
- 行为验证优先于实现细节验证
- 失败信号必须可解释、可定位、可复现

## 什么是伪覆盖
- 只执行文件加载或组件渲染，但不验证关键行为
- 只断言 mock 被调用，而不验证最终输出、状态变化或协议结果
- 只覆盖简单 happy path，却绕过真实边界条件、错误路径和数据形态
- 通过大量 stub、假数据或手写短路逻辑，让测试失去对真实链路的约束力
- 为提高 coverage 数字而给非关键分支补机械断言，却不提高实际信心

## 什么是真实覆盖
- 能覆盖用户真正依赖的核心能力：解析、格式化、视图构建、Worker/WASM 调用、关键 UI 交互
- 能验证输入到输出的关键链路，而不是只验证局部中间态
- 能覆盖成功路径、失败路径、边界输入和回归风险点
- 断言的是对外可观察结果：返回值、渲染结果、诊断信息、错误响应、协议结构、状态变化

## 覆盖优先级

### P0：必须覆盖
- `packages/core` 的核心计算链路
- 文档运行时主链：`packages/core/src/document/protocol.rs`、`packages/core/src/wasm_document.rs`
- `apps/web/src/workers` 与 `packages/core/wasm` 之间的 Job API / Snapshot Query / 错误通道
- JSON streaming、graph delta、parse-failed diagnostics-only、stale 隔离
- 编辑、格式化、构建视图、TreePath、关键图形交互等主用户路径

### P1：应优先覆盖
- 高复杂度模块、频繁改动模块、历史上容易回归的模块
- 多格式支持下共享主链路的分发逻辑
- 关键设置项、缓存逻辑、增量更新逻辑

### P2：可降权覆盖
- 展示性样式细节
- 纯转发型薄封装
- 低风险、低复用、可由上层核心链路自然覆盖的辅助代码

## 分层测试策略

### Core
- 优先用 `cargo nextest run --locked` 覆盖解析、格式化、算子、评估、View 构建等核心能力
- 优先写能直接验证输入输出与诊断结果的测试
- 共享主链路变更时，补回归测试而不是只补局部 helper 测试

### Web 单元测试
- 放在 `apps/web/src/**/*.test.ts`
- 适合验证纯前端状态逻辑、组件行为与局部交互
- 不要把本应由集成测试验证的 Worker/WASM 行为拆成过度 mock 的单元测试

### Web 集成测试
- 放在 `apps/web/test/integration/**/*.test.ts`
- 用于覆盖 Worker、WASM、协议、格式处理和跨模块协作链路
- 如果一个功能的价值依赖真实运行时，这一层比单元测试更重要

### E2E
- 放在 `apps/web/test/e2e/`
- 用于验证真实用户路径是否可完成，不承担所有边界分支覆盖
- E2E 数量应少而关键，避免把大量低价值细节堆到浏览器层

## 断言要求
- 断言必须指向业务结果或系统边界结果
- 优先断言结构化输出、视图结果、错误对象、诊断内容、状态变更
- 少做脆弱断言：内部调用次数、无业务意义的快照、偶然顺序和样式噪声
- 快照只能作为辅助手段，不能代替关键行为断言

## Timeout 与慢链路约束
- 所有测试、测试 helper 与测试框架级 timeout 必须 `<= 5_000ms`
- 超过 5 秒优先视为实现 bug、性能退化或异步时序问题，先修链路，不靠继续放宽 timeout 过测
- 禁止通过调大 timeout 来规避测试超时；遇到超时必须先 review 相关链路代码，分析超时根因（如不必要的串行等待、重复渲染、Worker 通信瓶颈、WASM 初始化延迟等），修复后再验证测试通过
- 对关键用户路径优先写预算型断言，让失败直接暴露慢链路，而不是长时间等待
- 如发现慢链路，先定位 Worker/WASM、图渲染、设置持久化、导入导出或浏览器交互链路，再决定是否需要拆分测试职责

## Mock 与替身规则
- 只在边界外部依赖不可稳定接入时使用 mock
- 对仓库内可真实调用的模块，优先走真实链路
- 如果 mock 会掩盖协议错误、序列化错误、运行时错误或关键状态转换，则不应使用

## UI 与 E2E 可测性
- 编写 UI 组件时，应主动提供稳定的语义化选择入口：`aria-label`、`role`、`title`
- 对关键节点补充 `data-testid`，尤其是画布容器、上传入口、弹层面板、复杂复合组件和非原生控件
- E2E 选择器优先使用语义属性与 `data-testid`，不要长期依赖 class 层级、文本碎片或 DOM 顺序
- 如果组件内部封装了 Monaco、Leafer 或其他复杂控件，应同时提供测试钩子或稳定容器标识，避免测试绕过真实 UI 主链路

## Leafer / Graph E2E 最佳实践
- 目标是验证真实用户路径与语义结果，不做像素级截图回归。
- 入口优先 `role`、`aria-label`、`title`、`data-testid`；画布统一从稳定容器进入，例如 `data-testid="graph-viewer-canvas"`。
- 等待对准业务完成信号：先 `waitForEditorReady`，更新内容后再 `waitForGraphRendered`，其余异步状态优先 `expect.poll`；禁止固定 `waitForTimeout`。
- 复杂画布交互优先通过测试 hook 或应用侧交互 API 驱动，断言 bounds、world 坐标、hit path、treePath、cursor、lastReveal、highlight、sourceText 等结构化结果。
- tooltip / graph hover panel 优先断言内容语义、`path`、`visible`、`rect` 与预览形态分发结果，不把 screenshot 作为主断言。
- 优先复用 `apps/web/test/e2e/utils.ts` 中的 editor、graph render、click probe、highlight、hover panel、runtime 查询 helper。
- 反模式：固定 sleep、键盘逐字输入 Monaco、直接改 store 代替真实交互、固定坐标点击画布、用 screenshot snapshot 替代业务断言。
- 参考用例：`apps/web/test/e2e/reveal-sync.spec.ts`、`apps/web/test/e2e/dom-component-migration.spec.ts`、`docs/user-stories.md`

## 覆盖率观念
- 覆盖率是辅助信号，不是目标本身
- 覆盖率提升若不能提升对核心链路的信心，视为低价值改动
- 对 `packages/core`，覆盖率应服务于核心能力稳定性，而不是平均摊薄到低价值分支
- 对 `apps/web`，可运行、能防回归的测试比单纯数字更重要

## 文档运行时回归矩阵

文档运行时回归测试只覆盖主文档链路的核心验收面，不为阶段或覆盖率数字补泛化测试。最低证据如下：

| 验收面                             | 最低证据                                                                                                | 当前主证据文件                                                                                                                                                                                  |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 模型与边界                         | 协议真源、WASM façade 与 TS 生成物仍一致                                                                | `packages/core/src/document/runtime.rs`、`packages/core/src/wasm_document.rs`、`apps/web/src/lib/services/document-job-stream.test.ts`                                                          |
| 语义所有权                         | runtime 持有 authoritative / transient / diagnostics-only / stale 语义；Worker 只做 transport / fan-out | `packages/core/src/document/runtime.rs`、`apps/web/src/lib/services/DocumentSessionService.test.ts`、`apps/web/src/workers/wasm-runtime.worker.test.ts`                                         |
| JSON streaming + graph delta | 大文件导入、streamed build、graph delta 不回退                                                          | `packages/core/tests/core_graph_builder_preorder.rs`、`apps/web/test/e2e/drop-import-regression.spec.ts`                                                                                        |
| hover 子图与 reveal/query          | hover subgraph、tree path、graph reveal 绑定同一 snapshot 语义                                          | `apps/web/src/lib/components/graph-viewer/graph-hover-panel.test.ts`、`apps/web/test/e2e/reveal-sync.spec.ts`                                                                                   |
| parse-failed / diagnostics-only    | parse failed 仍产出 diagnostics-only 结果并清理 graph 可见状态                                          | `apps/web/src/lib/services/DocumentSessionService.test.ts`、`apps/web/src/lib/components/Editor/editor-analysis-controller.test.ts`、`apps/web/test/e2e/invalid-json-graph-diagnostics.spec.ts` |
| 双向编辑与增量链路                 | 编辑器改动、图上编辑回写、fallback 仍收敛到统一 snapshot 主链                                           | `apps/web/test/e2e/bidirectional-edit-sync.spec.ts`、`apps/web/test/e2e/graph-edit-blur-commit.spec.ts`                                                                                         |

- `packages/core/src/wasm.rs` 的旧 ABI 兼容需验证，但它不是文档协议主链的长期真源。
- Web graph stream 性能口径已冻结在 `docs/web-graph-stream-benchmark.md` 与 `apps/web/benchmarks/graph-stream-baseline.json`。
- 回归比较统一执行 `cd apps/web && pnpm bench:graph-stream`：`successRate` 不允许下降；throughput 的 `avg(timeToGraphAppliedMs)` 恶化超过 `max(35%, 75ms)`、smoothness 的 `avg(maxFrameGapMs)` 恶化超过 `max(35%, 16ms)`、或 `avg(longFrameCount)` 增加超过 `2`，都视为回归。

## 新增或修改功能时的要求
- 改动核心行为时，必须补对应层级的真实测试
- 修 bug 时，优先补能稳定复现原问题的回归测试
- 变更协议、格式路由、错误处理或缓存逻辑时，必须覆盖失败路径与边界输入
- 如果选择不测某条路径，应能说明该路径为何不是核心链路，或已被更高价值测试自然覆盖

## 禁止事项
- 禁止为了通过 CI 人工弱化断言
- 禁止只增加 coverage 数字而不增加实际约束
- 禁止把所有风险都下沉给 E2E，导致单元和集成层失去价值
- 禁止长期保留失真测试：测试通过但不能代表真实运行结果

## 验证命令
- `packages/core`：`cargo nextest run --locked`
- `packages/core` fixture corpus：`cargo nextest run --locked --test corpus_runner --no-capture`
- `apps/cli` CLI unit tests：`cd apps/cli && cargo nextest run --locked --lib`
- `apps/cli` bash acceptance：`cd apps/cli && bash tests/acceptance/run.sh`
- `apps/web` 全量测试（单元 + E2E core）：`pnpm test`
- `apps/web` 全量+（全量 + 日常 E2E + fixtures E2E）：`pnpm test:all`
- `apps/web` 分层：`pnpm test:unit`、`pnpm test:integration`、`pnpm test:coverage`、`pnpm test:wasm`
- `apps/web` E2E：`pnpm test:e2e`、`pnpm test:e2e:headed`、`pnpm test:e2e:fixtures`、`pnpm test:e2e:fixtures:headed`、`pnpm test:e2e:core`、`pnpm test:e2e:core:headed`

## AI 执行约定
- agent 运行 `packages/core` 测试时，统一使用 `cargo nextest run`（替代 `cargo test` 以避免编译全部集成测试文件）。例外：WASM 测试、需要在单进程内验证非线程安全行为的场景仍需使用 `cargo test`。fixture corpus 也使用 `cargo nextest`，项目级并发限制由 `nextest.toml` 控制。
- 对 `apps/cli/src/**/*.rs` 的修改，优先运行 `cd apps/cli && cargo nextest run --locked --lib`
- 对 `apps/cli/tests/acceptance/**/*.sh` 的修改，或对会影响真实命令行外部行为的 CLI 改动，优先运行 `cd apps/cli && bash tests/acceptance/run.sh`
- 对 `apps/web/src/**/*.test.ts` 的修改，优先运行 `pnpm test:unit`
- 对 `apps/web/test/integration/**/*.test.ts` 的修改，优先运行 `pnpm test:integration`
- 对 `apps/web/test/e2e/**/*.spec.ts` 的修改，优先运行 `pnpm test:e2e`
- 需要 `apps/web` 仓库级确认时，先运行 `pnpm test`，需要包含浏览器链路时再运行 `pnpm test:all`
- 测试失败时，先判断该测试是否覆盖真实场景、核心链路或已知回归风险；若测试代表真实运行结果，优先定位并修复代码问题，不得通过弱化断言、改写测试意图或绕过失败来消除报错；若测试不代表真实场景，先修正测试，使其恢复对真实行为的约束
- 需要通过 E2E 方式做临时调试或验证时，优先使用 `agent-browser` SKILL 完成浏览器交互、观察与取证，不要为一次性确认新增长期保留的 E2E 用例
- Monaco Editor 的文本输入在 E2E 中必须通过应用侧自定义测试钩子完成，不得使用键盘逐字输入、textarea 直写或直接改 store 来替代真实编辑器赋值
- 需要文件导入或 compare 右侧加载文件时，优先走“点击触发入口 → 定位 file input → setInputFiles” 的 helper，避免在 E2E 中直接构造 `DataTransfer`
