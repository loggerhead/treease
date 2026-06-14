# Web Graph Stream Benchmark

## 目的

- 比较 `apps/web` 真实页面链路下不同 graph stream chunk size 的表现。
- 输入固定来自仓库根目录 `test/fixtures/{json,toml,yaml}` 的 raw valid fixture。
- 输出不是单个 case 的快慢，而是 bucket-level 的 throughput / smoothness 推荐与回归比较结果。

## 冻结口径

- 候选 chunk size：`16KB`、`32KB`、`64KB`、`128KB`、`256KB`（512KB 经测试与 256KB 持平，无额外收益）
- fixture 选择：每个体积 bucket、每种语言各取 **1 个最大 valid raw fixture**
- 当前 bucket：`<256KB`、`256KB-1MB`、`1MB-4MB`、`4MB-8MB`、`>=8MB`
- 长帧阈值：`50ms`
- 超长帧阈值：`100ms`
- smoothness 推荐先按 long frame 数量排序，再按最大 frame gap 排序；避免单次 max-gap 抖动盖过持续卡顿信号。
- long-frame count 回归容忍度为 4 帧；该指标在单 fixture / 单语言 bucket 上抖动较大，但仍会与 throughput、max frame gap 一起作为门禁。
- 单 case 超时：`45s`

## 运行命令

```bash
cd apps/web && pnpm bench:graph-stream
```

- 命令会先执行 `pnpm wasm:sync`，再分别以 3 个候选 chunk size 启动 Vite dev server、驱动真实 Chromium 打开 `/editor` 页面并采样。
- 命令结束后会把当前结果与冻结基线比较；若超出阈值，会以非零状态退出。
- 需要显式刷新基线时运行：

```bash
cd apps/web && pnpm bench:graph-stream -- --update-baseline
```

## 输出文件

- 最新运行：`apps/web/.tmp/graph-stream-benchmark/latest.json`
- 人工摘要：`apps/web/.tmp/graph-stream-benchmark/latest.md`
- 冻结基线：`apps/web/benchmarks/graph-stream-baseline.json`

`latest.json` 的固定顶层字段为：

- `schemaVersion`
- `generatedAt`
- `runner`
- `thresholds`
- `fixtureInventory`
- `candidateResults`
- `bucketSummaries`
- `recommendations`
- `comparison`

后续优化前后都以这套 schema 对比，不再用临时表格或手工口径。

## 关键指标

- `timeToFirstPartialMs`
- `timeToDoneMs`
- `timeToGraphAppliedMs`
- `chunkCount`
- `progressEventCount`
- `applyDeltaCount`
- `maxApplyDeltaMs`
- `maxFrameGapMs`
- `p95FrameGapMs`
- `longFrameCount`
- `veryLongFrameCount`

## 推荐规则

- throughput：先看 `successRate`，再看 `avg(timeToGraphAppliedMs)`，再看 `p95(timeToGraphAppliedMs)`
- smoothness：先看 `successRate`，再看 `avg(maxFrameGapMs)`，再看 `avg(longFrameCount)`，再看 `avg(maxApplyDeltaMs)`
- 若 throughput winner 与 smoothness winner 不同，报告 `split-recommendation`

## 回归阈值

`pnpm bench:graph-stream` 读取 `apps/web/benchmarks/graph-stream-baseline.json` 后，按 bucket winner 比较：

- `successRate` 不允许下降
- throughput：`avg(timeToGraphAppliedMs)` 恶化超过 `max(35%, 75ms)` 视为回归
- smoothness：`avg(maxFrameGapMs)` 恶化超过 `max(35%, 16ms)` 视为回归
- smoothness：`avg(longFrameCount)` 增加超过 `2` 视为回归

当前冻结的是 v2 基线文件：`apps/web/benchmarks/graph-stream-baseline.json`。

## 当前冻结推荐（v2）

基于 2026-06-02 benchmark（5 候选 x 3 语言 x 4 bucket，>=8MB 首次稳定跑通）：

- `<256KB`：throughput `128KB`（132.6ms），smoothness `16KB`（30.9ms）
- `256KB-1MB`：throughput + smoothness 双胜 `64KB`（3928.7ms / 1075.0ms）
- `1MB-4MB`：throughput `256KB`（5766.9ms），smoothness `64KB`（1208.4ms / 18 long frames）
- `>=8MB`：throughput `256KB`（7250.8ms），smoothness `128KB`（408.3ms）

**生产默认值已更新为 128KB**，理由：
- 全 bucket 覆盖下最均衡的单值选择
- >=8MB 比 64KB 提速 30%，1MB-4MB 提速 12%
- <256KB 与 64KB 持平（132.6ms vs 136.2ms）
- 256KB-1MB 略慢于 64KB（5518.5ms vs 3928.7ms）但仍在可接受范围

## 动态 chunk size

Graph stream chunk size 由 `apps/web/src/lib/graph-stream/chunk-size-policy.ts` 统一决定，normal GraphViewer render 与 same-language file import graph job 共用同一口径：

| 输入大小 | chunk size | 依据 |
|---|---|---|
| < 256KB | 128KB | 吞吐最优（124.2ms），smoothness 接近 |
| 256KB - 1MB | 64KB | 吞吐+smooth 双胜（4033ms / 1092ms） |
| 1MB - 4MB | 128KB | 吞吐接近最优（5675ms），smooth 最优（1192ms） |
| >= 4MB | 256KB | 吞吐显著领先（>=8MB 仅 7499ms），smooth 持平 |

Readable file import graph path 发送 `BinaryChunk`，避免浏览器主线程先 UTF-8 decode 再跨 Worker/WASM 边界传 `TextChunk`。内存全文渲染仍发送 `TextChunk`，因为 source 已经是 JS string。

## 风险标记

- `insufficient-fixtures`：当前 bucket 没有足够样本，不能给稳定建议
- `low-confidence`：当前 bucket 的有效样本数过少，结论可信度低
- `low-language-diversity`：当前 bucket 的语言覆盖过窄
- `json-only`：当前 bucket 只剩 JSON raw fixture 可用
- `split-recommendation`：throughput 与 smoothness winner 不同

## 仍保留的边界

- `4MB-8MB`：仓库 raw corpus 当前没有样本，这个 bucket 只记录为空，不参与阈值结论。
- `>=8MB`：当前只有 `jsonexamples__semanticscholar-corpus.1.json` 一个 raw 样本，而且 3 个候选 chunk size 都没有稳定成功结果；该 bucket 继续保留为观察项，不视为“优化已完成”。
- `1MB-4MB`：当前 bucket 只有 2 个样本，且仍存在 50% 成功率现象；winner 已冻结，但只能作为回归监控口径，不能被表述成“所有大文档都已稳定优化”。
