# 流式导入链路

本文只覆盖“输入如何变成 snapshot / graph delta”。

## 先记住的结论

| 链路 | 当前状态 | 测试时应如何理解 |
| --- | --- | --- |
| JSON same-language 文件导入 | 真流式 | chunk 期间可以产出 `ProjectionDelta` |
| 非 JSON chunk 导入 | 假流式 | chunk 期间只缓存；`close` 后才出结果 |
| CLI / 旧 API | 非流式 | 一次性全文输入，不代表 Web DocumentJob 主链 |

## 逻辑链路

### 1. JSON 真流式

```text
same-language File.stream()
  → startReadableDocumentJobSessionForGraph()
  → runReadableDocumentJobForGraph()
  → Worker advanceDocumentJob(chunk)
  → Rust advance_job()
  → advance_streaming_text_chunk() / advance_streaming_binary_chunk()
  → advance_streaming_chunk()
  → streaming_json::StreamDecoder.feed*
  → Builder.push(event)
  → StreamingGraphProjector.update()
  → DocumentEvent::ProjectionDelta
```

关键事实：

- `runIntakeJob()` 不是 same-language 文件流式主链；它更多服务于 full-edit / converted import。
- same-language 文件导入会把 `File.stream()` tee 成“刷新编辑器文本”和“推进 DocumentJob”两路。
- JSON streaming 期间 graph 输出由 `StreamingGraphProjector` 管理；事件是单调追加/更新，不靠 close 后再补一份“真正结果”。
- `parser.enableNest` 生效时，JSON decoder 会在 Core 侧直接把字符串化 nested JSON 展开成事件并重绑到外层 path；chunk 期 `ProjectionDelta`、close 后 `SnapshotReady.sourceText` 与后续 snapshot-bound 查询必须看到同一套 nested path。
- pending empty sequence / pending header-table schema 在 presentation 稳定前不应提前暴露成错误 graph node。

### 2. 非 JSON 假流式

```text
textChunk / binaryChunk
  → advance_job()
  → append_source_text / append_source_bytes
  → 返回空 batch
close
  → materialize()
  → decode(full_source)
  → build analysis / graph
```

这只是 transport chunking，不是 parser-level streaming。

### 3. CLI / 旧 API

```text
read full input
  → CodecService / evaluator
  → 可能一次性 parser.feed(full_source)
```

即使底层复用了 streaming decoder，只要调用方式是“一次性全文 feed”，就不算当前产品语义里的真流式。

## 链路节点复杂度

符号：`N`=源码字节数，`Q`=当前 chunk 事件数，`K_total`=当前投影作用域内完整 TreeStore 节点数，`G_total`=当前投影作用域内完整 graph model 节点数，`E_total`=当前投影作用域内完整 graph 边数，`ΔK`=本次增量触达的 TreeStore 节点数，`ΔG`=本次增量新增/更新/删除的 graph 节点数，`ΔE`=本次增量新增/更新/删除的 graph 边数，`D`=嵌套深度，`L`=行数，`H`=layout checkpoint 之后的重放长度，`dirty`=dirty handles / rows / edges 集合规模，`incident_edges`=与变更节点相邻且需要刷新 bounds/曲线的边。

| 节点 | 当前复杂度 | 说明 |
| --- | --- | --- |
| JS 侧切块 / coalesce | `O(N)` | 文本切 UTF-8 chunk，总体仍要走完整输入 |
| Rust `feed_json_chunk` | `O(chunk + Q log L + builder_cost)` | 含 source 累积、decoder、event drain |
| `Builder.push(event)` | `O(D)` / event，整体约 `O(K_total×D)` | 维护 tree、span、path 相关索引 |
| 首次 graph 投影 | `O(K_total + G_total + E_total)` | full topology + materialize + topology-backed full layout |
| 后续 chunk graph 更新 | 典型 `O(dirty + H + incident_edges)`，最坏 `O(G_total+E_total)` | changed-region relayout；edge 去重与 incident edge 查询走持久 index |
| 成功 close（JSON 流式） | `O(N + K_total + L + G_total + E_total)` | 收尾 analysis / line index / incremental state；不再通过 close 修正 chunk 期间 graph 语义 |
| 非 JSON close | `O(N + K_total + G_total + E_total)` | 假流式的主要成本集中在 close；source decode/parse 仍是全文 |

## 产品预期（可直接转成 core test）

- JSON chunk feed 在 `close` 之前就能产出 `ProjectionDelta`。
- `patchSeq`、`graphVersion` 单调前进；不能出现“先发旧版本、close 再纠正”的倒退语义。
- pending header-table / open empty sequence 在未稳定前不应提前发布成错误 node。
- 非 JSON chunk feed 在 `close` 前不应伪装成已完成 graph 构建。
- parse failed 时允许进入 diagnostics-only，但不能保留看似成功的旧 graph 可见结果。
- JSON 流式 close 后的最终 graph 语义应与同文本一次性全量构建一致。
- same-language 文件流式导入与 memory full-edit 导入最终 snapshot 语义一致。
- 流式增量 delta 只能 add/update 当前稳定信息，不能靠 remove 来“修正早先错误发布”的 pending table/node。

### 写 core test 时不要断言

- 不要断言具体 helper 名字或内部函数是否被调用。
- 不要把“使用了某个 decoder 类型”当作产品结果。
- 不要用最终 snapshot 正确来掩盖 chunk 期间没有真实流式行为。

## 推荐测试切面

- `DocumentEvent` 序列：是否在 `close` 前出现 `ProjectionDelta`。
- `SnapshotReady.mainGraph`：是否保持 `clear` / `graphVersion` / graph 数据语义正确。
- 同一输入的 streaming vs full-build：最终 graph 是否等价。
- 错误输入：是否进入 diagnostics-only，而不是 silent success。
