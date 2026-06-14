# 双向编辑链路

本文覆盖 Editor → Graph 与 Graph → Editor 两条路径，以及它们如何在 snapshot 语义上重新收敛。

## 先记住的结论

- Editor → Graph 的对外入口是 `commitDocument(...edits...)`，Core 内部统一落到 `ApplyEdits` job。
- Graph → Editor 不是直接改文档；它先做 snapshot-bound planner，再把结果回送成 `DocumentTextEdit[]` 或显式 fallback `replace`。
- 支持增量时要说明是哪一层增量：Monaco text edit、tree-sitter syntax incremental、或 structural incremental。

## 1. Editor → Graph

### 提交链路

```text
Monaco onDidChangeModelContent
  → monacoChangesToDocumentTextEdits(...)
  → DocumentTextEdit[]
  → commitDocument({ text, edits, baseSnapshotId, ... })
  → startSharedGraphDocumentJob(... kind = ApplyEdits ...)
  → Worker startDocumentJob / advanceDocumentJob(close)
  → Rust materialize_with_base()
```

### Core 内部三层降级

```text
ApplyEdits
  → try_structural_materialize
    │ hit  → structural incremental
    │ miss ↓
  → syntax incremental fallback（prepared ts_tree）
    │ hit  → full decode + prepared tree analysis + full graph
    │ miss ↓
  → full rebuild fallback
```

### 当前已实现范围

| 能力 | 当前状态 |
| --- | --- |
| JSON / YAML / TOML / Python / JavaScript 安全单标量 value edit | structural incremental |
| JSON / YAML / TOML / Python / JavaScript 安全 key edit | structural incremental |
| JSON 非 root object/array subtree edit | 可局部 decode + graft |
| CSV 单 cell value edit | structural incremental |
| CSV header key rename | 显式 fallback |
| 多 edit / root boundary / span-path 不安全 / 复杂 subtree | full rebuild fallback |

### base snapshot 可复用资产

- `analysis.document`
- `analysis.line_index`
- `analysis.ts_tree`
- 节点 source span
- `snapshot.incremental.can_resume`
- `snapshot.incremental.graph_model` / `graph_model_index`
- `snapshot.incremental.structural_span_index`

## 2. Graph → Editor

### 规划链路

```text
Graph inline edit
  → commitTextEdit
  → Worker handlePlanGraphValueEdit
  → Core/WASM plan_graph_value_edit(documentKey, snapshotId, language, path, preferKey, value)
```

### planner 语义

- planner 必须绑定 `documentKey + snapshotId + path`。
- `DocumentSnapshot::plan_graph_value_edit()` 负责 snapshot / document identity 校验。
- 具体语言规则真源是 `LangSpec.graph_value_edit_rule`。
- planner 返回 `mode: "edits"` 时，UI 直接应用 `DocumentTextEdit[]`，随后这些 edit 重新进入上面的 Editor → Graph `ApplyEdits` 主链。

### 当前覆盖

| 语言 | value edit | key edit |
| --- | --- | --- |
| JSON | 支持 | 支持 |
| YAML | 支持 | 支持 |
| TOML | 支持 | 支持 |
| Python | 支持 | 支持 |
| JavaScript | 支持 | 支持 |
| CSV | cell value 支持 | key 不支持，fallback |

### fallback

以下场景必须显式回到 compat `replace`：

- snapshot not ready
- missing analysis / document
- unsupported language / edit
- invalid path / replacement
- unsafe edit
- CSV key edit

## 链路节点复杂度

符号：`N`=源码字节数，`K_total`=当前投影作用域内完整 TreeStore 节点数，`G_total`=当前投影作用域内完整 graph model 节点数，`E_total`=当前投影作用域内完整 graph 边数，`ΔK`=本次增量触达的 TreeStore 节点数，`ΔG`=本次增量新增/更新/删除的 graph 节点数，`ΔE`=本次增量新增/更新/删除的 graph 边数，`L`=行数，`D`=深度，`R`=replacement 长度，`P`=path sibling 扫描量，`incident_edges`=与变更节点相邻且需要刷新 bounds/曲线的边。

### Editor → Graph

| 节点 | 当前复杂度 | 说明 |
| --- | --- | --- |
| source apply（单 edit） | `O(N + R)` | 当前连续 `String` 模型复制 prefix/replacement/suffix；本轮 graph 优化不改变 source buffer 下界 |
| structural incremental（当前最佳路径） | 典型 `O(N + ΔK log L + ΔG + incident_edges)`；最坏仍可 fallback 到 full rebuild | snapshot incremental state 持久保存 graph/layout index；source apply 仍是连续文本复制 |
| syntax incremental fallback | `O(N + K_total + G_total + E_total)` | prepared tree 只省解析常数，不改全量 decode / graph 渐进量级 |
| full rebuild fallback | `O(N + K_total + G_total + E_total)` | 最保守路径 |

### Graph → Editor

| 节点 | 当前复杂度 | 说明 |
| --- | --- | --- |
| snapshot-bound planner | 典型 `O(path_key_bytes + R)` | 命中 path/span index 时很快 |
| planner 最坏情况 | `O(N + K_total + R)` | index 缺失或 span 缺失时退化 |
| compat replace fallback | `O(2N + 2K_total + P + R)` | 一次 full decode/encode，再附带 parse-to-tree 语义成本 |

## 产品预期（可直接转成 core test）

- 支持的安全标量 / key edit 应走统一 `ApplyEdits` 语义收敛，结果 graph 正确，且 `mainGraph.clear = false`。
- structural miss 时可以 fallback，但必须在同一个 `ApplyEdits` job 内显式完成，不能对外伪装成增量成功。
- Graph → Editor planner 必须校验 snapshot/document identity；旧 snapshot 或错文档不能静默成功。
- planner 返回 `edits` 时，这些 edit 再进入 Editor → Graph 后，最终结果应与直接全量替换后的文档语义一致。
- 未覆盖场景必须显式 `mode: "replace"` 或记录 fallback reason，不能 silent no-op。
- JSON 非 root subtree edit 命中局部 graft 时，最终 graph 应只在受影响子树变化。
- CSV cell value edit 支持局部回写；CSV key edit 不支持时要稳定 fallback。
- value edit 与 key edit 的语言矩阵要分别验证，不能只测 JSON happy path。
- Graph 上编辑某个 path 后，文档文本、snapshot、graph 再次对该 path 查询应保持一致。

### 写 core test 时不要断言

- 不要断言内部一定经过某个 helper 或某级 fallback。
- 不要把“有增量状态对象”当作用户可见成功条件。
- 不要只测 planner 返回了 edit；必须继续验证这些 edit 应用后的最终文档 / graph 语义。

## 推荐测试切面

- 语言矩阵：JSON / YAML / TOML / CSV / Python / JavaScript。
- 类型矩阵：value edit、key edit、subtree edit、unsupported edit。
- snapshot 矩阵：active snapshot、stale snapshot、wrong documentKey。
- 收敛矩阵：Graph → Editor 规划出的 edit 重新流入 Editor → Graph 后，结果是否与目标语义一致。
