# Graph Layout 链路

本文覆盖 graph node 如何从结构树生成、如何布局，以及 streaming / full build 如何共享同一套 authority。

## 先记住的结论

- 坐标 authority 真源是 `core::layout_engine::LayoutEngine`。
- graph node / dirty authority 真源是 `core::graph_topology::GraphTopology`。
- full build 与 streaming changed-region layout 不能各自维护一套不同语义。

## 节点生成规则

### TreeNode → Graph node

| TreeNode | Graph node |
| --- | --- |
| `Mapping` | `Object` |
| `Sequence` | `Table` |
| 其他（`Scalar`、`Alias` 等） | `Scalar` |

### 主图中哪些节点会真正出现

- 非根空 `Mapping/Sequence`、标量、alias：不生成独立 graph node，值内联在父节点。
- headerless sequence 中的非空 `Mapping/Sequence` item：生成子 graph node。
- header-table row：始终 folded 在父 table 内；row 本身不是主图 node。
- row 内 nested container cell：作为悬停预览入口，而不是主图 row node。
- empty → non-empty 容器升级：只允许 add/update，不允许先错误发布再 remove 纠正。

### 节点内部结构

- `Object`：每行 `key/value` 两列；空对象显示一行 `(empty)`。
- `Table(with header)`：第一项是 `Mapping` 时触发；列集合是对象 key 的并集，带索引列。
- `Table(without header)`：第一项不是 `Mapping` 时触发；每行只有索引和值。
- `Scalar`：单行，key 为 `"value"`。

## 布局规则

### X 坐标

- 根节点 `x = 0`。
- 同一深度所有节点对齐到同一列。
- 某层列坐标 = 上一层所有节点最右边界最大值 + `h_gap`。
- 结果上等价于“同层子节点共享一列”，而不是每个父节点独立推导自己的子列。

### Y 坐标

- 某深度第一个节点：`y = 父节点 y`。
- 同层后续节点：`y = max(父节点 y, 该层已放置子树底部 + v_gap)`。
- streaming 增量路径通过 `layout_changed_region` 复用同一权威规则，不维护第二套近似位移逻辑。

### edge 几何

- edge 为贝塞尔曲线。
- 起点 `x` = 父节点右边界，终点 `x` = 子节点左边界。
- 起点 `y` = 父节点对应 value 单元格中点。
- 终点 `y` = 子节点首个 row 中点。

## full / streaming 入口

```text
full build
  → GraphTopology.build_full
  → materialize_into_current_model
  → LayoutEngine.layout_full

streaming / changed subtree
  → GraphTopology.apply
  → materialize_into_current_model
  → LayoutEngine.layout_changed_region
  → StreamingDeltaDiffer
```

要点：

- full build 与 streaming 最终共享 `GraphTopology` + `LayoutEngine` 语义。
- `finalize_layout()` 在 close 路径收尾；如果没有额外改动，它可以是 no-op。
- table row/cell bounds 由表格 materialize 的增量路径维护，不应在 growing table 时每个 chunk 全表重排。

## 链路节点复杂度

符号：`K_total`=当前投影作用域内完整 TreeStore 节点数，`G_total`=当前投影作用域内完整 graph model 节点数，`E_total`=当前投影作用域内完整 graph 边数，`ΔK`=本次增量触达的 TreeStore 节点数，`ΔG`=本次增量新增/更新/删除的 graph 节点数，`ΔE`=本次增量新增/更新/删除的 graph 边数，`D`=深度，`H`=layout checkpoint 后缀长度，`dirty`=dirty handles / rows / edges 集合规模，`incident_edges`=与变更节点相邻且需要刷新 bounds/曲线的边。

| 节点 | 当前复杂度 | 说明 |
| --- | --- | --- |
| `GraphTopology.build_full()` | `O(K_total + G_total + E_total)` 量级 | full DFS 使用 path stack；不再为每个 graph node 递归重建 root→node path |
| `GraphTopology.apply()` | `O(patches×D + ΔK)`，最坏 `O(K_total)` | 对 patch anchor、祖先、受影响子树做 reconcile |
| `materialize_into_current_model()` | 首次 `O(G_total + table_rows + E_total)`；增量目标 `O(ΔG + dirty_table_rows + ΔE)` | edge 去重走持久 `GraphModel` edge index，不再每次增量扫描全量 edges |
| `LayoutEngine.layout_full_with_topology()` | `O(G_total + E_total)` | 使用 `GraphTopology` children adjacency 与持久 edge index；legacy `layout_full()` 仍保留给无 topology 的旧调用方 |
| `LayoutEngine.layout_changed_region()` | 典型 `O(seed + H + incident_edges)`，最坏 `O(G_total+E_total)` | 只刷新 changed bounds 与 seed/新增边的 incident edge 几何 |
| `StreamingDeltaDiffer.emit_incremental_delta()` | `O(ΔG + layout_edges)` | 把拓扑 / 布局变化转成协议 delta |

## 产品预期（可直接转成 core test）

- 相同结构的 full build 与 streaming build，最终 graph node 类型、父子关系、table 表现必须一致。
- header-table 与 headerless sequence 的判定必须稳定，不能因增量路径而出现不同呈现。
- 非根空容器、header-table row、inline scalar 不应错误升级成主图 node。
- empty → non-empty 容器升级只能 add/update，不能依赖 remove 纠错。
- changed-region layout 之后，不受影响节点的相对语义不应被破坏。
- 同深度列对齐：兄弟子树的 `x` 应满足同层对齐规则。
- `y` 坐标应保持父先子后、层内不重叠。
- edge 的起终点应绑定正确的父 value row 与子首 row，而不是任意节点中心点。
- table 增长时允许增量扩展 bounds，但不应因为每个 chunk 重排整个表而造成语义抖动。

### 写 core test 时不要断言

- 不要把具体像素值写死成脆弱快照；优先断言相对关系与结构不变量。
- 不要把“某一步经过 full layout 还是 changed-region layout”当作对外语义。
- 不要把 row/cell 内部临时 materialize 顺序当作产品契约。

## 推荐测试切面

- 节点类型矩阵：object / table(with header) / table(without header) / scalar。
- 父子边矩阵：主图 node 是否只出现在允许的位置。
- 坐标不变量：同层 `x` 对齐、`y` 非重叠、父子边方向正确。
- full vs streaming 对照：同输入最终 graph / layout 是否等价。
