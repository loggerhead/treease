---
summary: "Executable /goal directive for scoping editor document operations, async landing, and cancellation to left tabs."
read_when:
  - Refactoring full-edit, import, whole-document replacement, or formatting across editor tabs
  - Changing ownership of editor DocumentJobs, full-edit UI state, or background-tab async work
---

# /goal：按 Tab 收敛编辑器文档操作生命周期

## 目标

将左侧编辑器 Tab 的 full-edit、文件导入、导入转换、whole-document replacement 和格式化收敛为一条按 `tabId` 归属、按 `documentKey + revision` 提交、可独立失效和清理的文档操作生命周期。

完成后必须同时满足：

```text
一个左侧 Tab 最多拥有一个当前文档操作；
同一 Tab 的新操作取代旧操作，不同 Tab 的操作互不取消、互不排队；
切换 Tab 不改变操作的文档归属；
关闭 Tab 会先同步失效其操作，再释放该 Tab 的异步资源；
任何异步结果只能提交到产生它的文档，只有 active Tab 的结果可以进入当前可见 Editor / Graph UI。
```

本目标延续 [统一编辑器 Tab 生命周期](./tab-lifecycle-unification-plan.md) 已确定的拓扑与 Monaco model 所有权，不新增第二套 Tab authority。

## 产品语义

### Switch

- 从 Tab A 切到 Tab B 时，A 的 full-edit、文件读取、格式转换或格式化继续执行；切换本身不得取消 A 的 `DocumentJob`。
- B 的 Editor、cursor、readOnly、Graph、diagnostics 和 active 全局投影只反映 B。
- A 可以继续更新自己的 resident Monaco model、workspace 文本镜像、revision、snapshot binding 和 tab-local operation UI state，但不得更新 B 或当前可见 Graph。

### Reactivate

- 切回仍在运行的 Tab A 时，不得重新启动、重复计费或替换 A 的当前操作。
- A 的 tab-local operation UI state 重新成为 active 投影；Editor 交互模式由 A 的操作状态恢复。
- Graph 不得把晚附着时取得的不完整 batch replay 当成完整图。若不能从可证明的完整 baseline/rebase 恢复流式图，则保持终态等待状态，直到 A 产生 authoritative terminal snapshot 后再渲染。

### Close

- 关闭 Tab A 时，必须先同步标记 A 的 operation generation 已失效，使所有晚到回调立即失去落地资格。
- 拓扑转换和 successor model 安装不得等待 Worker 网络往返或 `cancelDocumentJob` 完成。
- 同步失效后，异步取消 A 的 conversion operation、RAF、stream reader 和 `DocumentJob`；每种资源最多清理一次。
- 只有 operation owner 可以取消 `DocumentJob`。Graph render attachment 的脱离不得取消仍由 Tab A operation runtime 持有的 job。
- 完成上述同步失效后，按既有 Tab close 契约发布 workspace、安装 successor、销毁 A 的 Monaco model。

### Same-tab supersede

- Tab A 发起新 whole-document operation 时，先同步失效 A 的旧 generation，再异步取消旧资源，然后启动新 generation。
- 旧 `sessionId`、`ownerKey`、`documentKey` 或 revision 的回调只允许完成幂等清理，不得覆盖新操作状态。

### Different tabs

- Tab A 与 Tab B 的操作使用独立 lifecycle 和队列。
- “跨 Tab 并行”指 Web 层不再用一个全局 Promise queue 制造 head-of-line blocking；底层 Worker 是否物理并行不属于本目标的承诺。
- 一个 Tab 的 file import 只能阻止该 Tab 的 format/minify/compact/sort，不得阻止其他 Tab 的命令。

## 状态所有权

### Workspace tab state

`EditorWorkspaceState.tabsById[tabId]` 是每个 Tab 的 Web 运行时状态事实来源，继续保存：

- `documentKey`、`languageId`、`sourceText`、`revision`、`graphAppliedRevision` 和 `snapshotId`；
- `tempModel`；
- `fullEditUiState`。

`fullEditUiState` 存在于 workspace runtime 不等于它可以持久化。`WorkspaceSession` 仍只保存现有草稿字段：Tab 名称、语言、文本、origin、saved text 和 active index。运行中的 session、owner、phase、Promise、Job 或 queue 不得进入 Browser/Desktop session 数据；恢复后的所有 Tab operation state 必须是 idle。

### Tab operation runtime

在 `EditorCore` 私有 runtime 中建立按 `tabId` 索引的 operation owner。它只拥有不可持久化资源：

- 当前 operation generation；
- full-edit/import session；
- import conversion `ViewRuntimeOperation`；
- `TextDecoder`、stream reader、RAF handle 和 pending chunk buffer；
- `FullEditDocumentJobSession` 的取消所有权；
- whole-document replacement 的异步语言检测；
- format command queue 和在途 Worker request；
- 幂等 cleanup 状态。

该 runtime 不得拥有 Tab 顺序、active/primary 选择、Tab 名称、workspace 持久化或 Graph 语义。

优先复用现有 `EditorFullEditController` 的单-session Implementation：可以为每个左侧 Tab 创建一个绑定稳定 target 的 controller/runtime entry，而不是把 controller 内每个字段机械改成多个并行 `Map`。无论采用哪种 Implementation，调用方只能通过一个深 Module 发起、查询、失效或释放某个 Tab 的操作。

### Active projection

以下状态只允许是 active Tab 的派生投影：

- 顶层 `fullEditUiState`；
- 当前 Monaco `model` 指针；
- Editor readOnly / interaction mode；
- 当前 cursor、selection、hover 和 JSON block selection；
- 当前 Graph render attachment 和 progress。

顶层 `fullEditUiState` 必须成为从 workspace active Tab 派生的只读 store。删除“先写全局 full-edit store，再由 coordinator 写回 primary Tab”的反向路径，以及无生产 contract 支持的直接 mutator。Tab-local sink 直接更新目标 workspace Tab；active projection不得写回 workspace。

## 稳定文档操作目标

每个异步文档操作在开始时必须捕获一个稳定 target，其语义至少包含：

```ts
type EditorDocumentOperationTarget = {
  tabId: string;
  model: Monaco.editor.ITextModel;
  ownerKey: string;
  documentKey: string;
  revision: number;
  languageId: SupportedEditorLanguageId;
  generation: number;
};
```

类型名和具体封装可以调整，但不得删减这些身份语义或在异步回调中重新读取 active Tab 来替代它们。

### Document freshness

一个 operation 只有同时满足以下条件才是 document-current：

- target Tab 仍存在；
- runtime 中该 Tab 的 generation 与捕获 generation 相同；
- target Monaco model 仍是该 Tab 的 resident model 且未 dispose；
- workspace Tab 的 `documentKey`、revision 和 language 与 target 一致；
- operation 未被显式取消。

`activeTabId` 不属于 document freshness。切换到另一个 Tab 不得令后台文档操作 stale。

### Visible freshness

结果进入当前可见 Editor / Graph / cursor / selection 还必须额外满足：

- `workspace.activeTabId === target.tabId`；
- `editor.getModel() === target.model`；
- 当前 active document context 的 `documentKey + revision` 与 target 一致。

Document freshness 和 visible freshness 必须是两个显式检查，不能继续共用“当前 model 是否仍 active”这一条件。

## Commit Transaction 契约

- 所有 primary-document 写入仍必须经过现有 `Commit Transaction`；本目标不得创建第二套 snapshot 或 semantic authority。
- 后台 Tab 的 current operation 可以提交 canonical source、Document Runtime terminal outcome 和 workspace snapshot binding。
- `EditorCommitTransaction` 的 document landing 与 visible landing 必须可区分：
  - document landing 写 target model/workspace mirror、semantic state 和 snapshot binding；
  - visible landing 只在 visible freshness 成立时更新 Graph scene、active diagnostics、semantic-token refresh、cursor 或 toast。
- `ParseFailed`、blank clear、`SnapshotReady` 和 snapshot-bound read 的语义继续完全归 Document Runtime。
- 对 target document 已 stale 的 result，document landing 和 visible landing 都不得执行。

错误必须写入目标 Tab 的 `tempModel`。只有错误或完成发生时目标 Tab 仍 active，才允许显示现有全局 toast；后台 Tab 的结果不得显示成当前 Tab 的成功或失败。

## DocumentJob 与 Graph attachment 所有权

现有 `fullEdit-document-job-session` 已按 `sessionId` 保存多个 session，可以继续作为 job registry；不得再新增功能重复的第二个 job registry。

必须修正当前 Graph attachment 对 job 的取消所有权：

- Tab operation runtime 是 full-edit/import `DocumentJob` 的唯一取消 owner。
- Graph View Runtime 只拥有 batch consumer / render attachment。
- active Tab 切换、Graph render freshness 变化或 Graph scene dispose 只能 detach consumer，不能调用该 session 的 `cancelDocumentJob`。
- same-tab supersede、Tab close、Editor dispose 或显式用户取消才可以取消 job。
- job cancel、session registry clear、RAF cancel 和 UI settle 必须分别幂等，不能依赖调用顺序避免 double cleanup。

`FullEditDocumentJobSession` 当前只保留有限 replay window。晚附着不得假定该窗口包含完整构图历史：

- 从 operation 开始即持续附着的 active Graph 可以继续消费 streaming batches；
- 中途 detach 后再次 attach，或首次晚附着，只能在有完整 baseline/rebase 证明时恢复 streaming；
- 本目标默认不实现新的完整 replay protocol。无法证明安全时等待 terminal result，再走 snapshot-bound canonical render；不得扩大 replay window 作为无界内存 fallback。

## Targeted whole-document replacement

建立一个以 target Tab 为参数的 canonical whole-document replacement Module，统一承接：

- format/minify/compact/sort 结果；
- import conversion 结果；
- language switch；
- preset、share restore、file replace 和其他 programmatic replace；
- active Editor 中识别到的 whole-document user replacement。

该 Module 必须一次捕获目标、文本、语言解析策略、source writeback policy、format-on-close 和 usage-metering policy。不得继续通过“全局 metadata 槽位 + active model change listener + 文本相等比较”恢复调用意图。

删除或按该 Module 吸收：

- `queuedWholeDocumentReplacement`；
- `queuedProgrammaticSourceText`；
- 只因 active full-edit 而延迟、但没有 `tabId/documentKey` 的 pending replacement。

whole-document replacement 必须按以下顺序执行：

1. 捕获目标 Tab 和 resident model；
2. 同步失效该 Tab 的旧 operation generation；
3. 通过窄的 target-document transition 分配新的 document identity 和 revision；
4. 更新 target model，并同步维护 workspace mirror；
5. 启动唯一 `Commit Transaction`；
6. 仅在 visible freshness 成立时更新当前可见 View Runtime。

`updateWorkspaceTab` 当前刻意不允许通用 patch 修改 inactive left Tab 的 language，并且纯 workspace patch 不允许覆盖 `documentKey`。不得放宽成任意字段 patch。应新增窄的、能校验旧 identity 的 target-document transition，用于原子更新指定 left Tab 的 documentKey、language、revision、source mirror 和清理旧 snapshot binding；sidecar 和普通 UI patch 不能调用它。

## Format command 契约

- format command 入队时捕获 target、source text、language、formatting options、nest setting 和 command kind。
- 同一 Tab 内保持命令顺序；不同 Tab 不共享应用层 Promise queue。
- Worker 返回后只调用 targeted replacement Module，不能读取完成时 active language 或 active model。
- 如果结果与捕获 source 相同，不启动 replacement 或 Commit Transaction。
- reset cursor 只在 target 仍 active 且 replacement 成功落地时执行；后台 Tab 完成不得移动当前 cursor。
- target Tab 已关闭、identity 已旋转、revision 已推进或 generation 已替换时，Worker 结果为 stale，只清理 queue entry。
- 当前底层 Worker 若串行处理请求可以继续串行；不得为追求物理并行引入第二个 Worker 或修改 WASM。

## Editor interaction 投影

- 删除 `addTab`、`activateTab`、`closeTab` 和 `openDocument` 对“任意 import 正在运行”的全局阻塞。
- Tab close 仍必须遵守既有 dirty-confirmation 和 topology contract。
- active Editor readOnly 必须从 active Tab operation 声明的 interaction mode 派生。后台 file import 不得锁住当前 Tab；切回仍在 file import 的 Tab 必须恢复 readOnly。
- 如果现有 `FullEditUiState` 不能封闭表达 interaction mode，应扩展其 discriminated state，而不是在 EditorCore 中根据多个 nullable 字段猜测。
- editor analysis、hover、semantic color refresh 等只检查 active Tab 是否正在阻止对应交互，不得查询“任意 Tab 是否有 import”。
- `waitForIdle()` 保持 active-document 语义，供 share restore 等现有调用等待当前 Tab。Editor dispose 另行取消并等待所有 runtime entries 的 cleanup；不要把两种等待语义混成一个全局轮询。

## Sidecar 边界

- 左侧 Tab operation runtime 不得接管右侧 sidecar topology 或把 sidecar 放入左侧 registry。
- `SidecarEditor` 继续使用自己绑定固定 `tabId/model/sink` 的 controller lifetime。
- 共享的 full-edit controller Interface 必须同时支持左侧稳定 target adapter 和现有 sidecar fixed-target adapter，但不能为了兼容保留 primary global sink 双写路径。
- 关闭或切换左侧 Tab 不得取消 sidecar job、修改 sidecar source、revision、snapshot 或 full-edit UI state。

## 明确不做

- 不修改 Core protocol、WASM Document Runtime 或 snapshot 语义。
- 不新增 workspace store、第二套 semantic authority 或第二个 full-edit job registry。
- 不持久化 Promise、Job、sessionId、ownerKey、operation phase 或 queue。
- 不通过无限增长 batch replay 恢复后台 streaming graph。
- 不在本目标中让 `jsonBlockSelection` 切回恢复；继续采用切 Tab 清空的现有产品语义。
- 不保证 Worker 物理并行，也不新增 Worker pool。
- 不改造 sidecar 为多 Tab。

## 实施顺序

1. 先建立失败反馈环，不改生产行为：
   - import A 后切到 B，证明当前 A job 被取消或 B 被全局阻塞；
   - format A 的 Worker 未返回前切到 B，证明结果可能写入 B；
   - close A during import，证明 cleanup owner 与调用次数；
   - A/B 各自启动 operation，证明当前单槽/全局 queue 行为。
2. 在 workspace/authority seam 增加窄的 target-document transition 和 active full-edit projection selector；为 inactive left Tab identity、language、revision、source 和 snapshot cleanup 写纯测试。
3. 让所有左侧 Tab 使用 workspace-tab full-edit sink；将顶层 full-edit store 改为 active Tab 的只读投影，删除 primary global sink 与 coordinator 双向写回。
4. 建立私有 tab operation runtime，先迁移 generation、失效、dispose 和资源 cleanup；保持 SidecarEditor 的 fixed-target lifetime 独立。
5. 修正 `FullEditDocumentJobSession` / Graph render attachment 所有权：Graph detach 不取消 job；补 late-attach terminal-render policy 和测试。
6. 将 full-edit/import 与 import conversion 迁移到稳定 target；所有 model、source、language、revision、tempModel 和 UI 更新改为 target-scoped。
7. 建立 targeted whole-document replacement canonical path，迁移 programmatic replace 与 active Editor whole-document event，删除两个无身份 queue 槽位。
8. 将 format scheduler 改为 per-tab ordering，并只通过 targeted replacement 落地。
9. 删除全局 import Tab-command guard；让 Editor interaction/readOnly/analysis/hover 使用 active Tab 投影。
10. 删除旧 global session/conversion/format queue、primary sink、双写 coordinator、兼容 mutator、旧 freshness 条件和只保护旧路径的测试。
11. 更新 [Editor Data Flow Contract](./contracts/editor-data-flow.md) 中 View Runtime Operation Lifecycle，记录 document freshness 与 visible freshness 的区别，以及 Graph attachment 不拥有 DocumentJob cancellation。

每一步完成后删除被替代路径，不允许长期双写或同时保留 global 与 tab-scoped 两套 lifecycle。

## 验收标准

只有同时满足以下条件才算完成：

- left Tab topology 仍只有 `EditorWorkspaceState` 一个事实来源；
- 每个左侧 Tab 最多一个 current operation generation；
- 所有异步回调都有稳定 `tabId/model/documentKey/revision/language/generation`；
- `activeTabId` 不参与后台 document freshness，只参与 visible freshness；
- Graph detach 不会取消后台 Tab 的 full-edit/import job；
- close 或 same-tab supersede 对每种资源最多 cleanup 一次；
- inactive Tab 的 source/revision/snapshot 可以正确提交，active Tab 的可见状态不被修改；
- 顶层 `fullEditUiState` 是只读 active projection，不再反向写 workspace；
- whole-document replacement 只有一个 targeted canonical path；
- format 同 Tab 保序、跨 Tab 无应用层 head-of-line blocking；
- session persistence 不包含运行时 operation 状态，restore 后全部 idle；
- primary global sink、全局 import topology guard、无身份 deferred replacement 和旧单槽状态均已删除；
- sidecar 行为和左侧生命周期继续隔离；
- 没有新增 Core/WASM 改动或 fallback path。

## 必须验证的行为

### Unit

- 更新 Tab A 的 full-edit state 时，Tab B 和 active projection 在 B active 时不变；激活 A 后投影精确等于 A。
- document freshness 在 A→B switch 后仍成立；visible freshness 对 A 变为 false。
- close/supersede 同步令旧 generation stale，job cancel、RAF cancel、session clear 和 UI settle 各执行一次。
- Graph render attachment detach 不调用 `session.cancel()`；operation owner close 才调用。
- late attachment 在没有完整 baseline 时不消费有限 replay window 作为完整构图，只等待 terminal canonical render。
- inactive left Tab 的窄 transition 可以旋转自己的 documentKey、更新 language/revision/source，并清理旧 binding；普通 patch 仍不能改 identity。
- converted import 在切 Tab 后写回原 Tab；foreign tab model/source/tempModel 不变。
- format 同 Tab 保序，A/B 各自入队互不等待应用层 queue；A 的结果只调用 A replacement。
- same-text format 不推进 revision；stale format 不写文本、不移动 cursor。
- active full-edit projection 只读；删除的 global mutator 不再有生产调用。

### Integration

- Tab A 开始 file import 并产生 ProjectionDelta，切到 B 后 A job 继续，B 可编辑且 readOnly 为 false。
- 切回仍在 import 的 A 时恢复 A 的 operation UI/readOnly；若没有安全 rebase，Graph 等待 terminal snapshot 而不显示缺失前序 batch 的错误 partial graph。
- A 在后台 terminal 后，A 的 model、workspace text、revision 和 snapshot binding 更新；B 的 current Editor/Graph/diagnostics 不变。
- 关闭后台 importing A 时立即完成 Tab topology transition；晚到 Worker result 不落地，job 最终被取消一次。
- active importing A 被关闭时，successor model 先安装；Graph detach 与 operation cleanup 不 double-cancel。
- A/B 同时有 full-edit/import operation 时，一个完成、失败或取消不改变另一个的 session state。
- format A 后立即切 B，A 完成后只有 A 文本变化；B cursor、language、revision 和 Graph 不变。
- whole-document replacement 对 inactive target 仍经过正常 `Commit Transaction`，而不是只修改 workspace mirror。
- Sidecar 运行 full-edit 时切换/关闭左侧 Tab 不影响 sidecar，反之亦然。

### End-to-end

- 文件导入 A 期间创建并激活 B：B 可输入，Tab A 显示自己的进行中状态，最终切回 A 得到完整文本和 authoritative graph。
- 文件导入 A 期间关闭 A：立即显示正确 successor；A 的晚到结果不改变 header、Editor、Graph 或 workspace binding。
- 在 A 点击 Format 后立即切到 B 并编辑 B：format 结果只出现在 A。
- A 与 B 分别发起格式化，均可完成且保持各自命令顺序。
- Browser/Desktop session 在 operation 运行时保存并重新加载，只恢复草稿字段；恢复后的 Tab 均为 idle，不尝试重连旧 sessionId/job。
- 打开 sidecar 后执行上述 switch/close/import 场景，sidecar state 始终不变。

## 验证命令

先执行最小相关检查：

```bash
cd apps/web
pnpm exec vitest run \
  src/lib/components/Editor/editor-full-edit-controller.test.ts \
  src/lib/components/Editor/editor-full-edit-sink.test.ts \
  src/lib/components/Editor/editor-format-controller.test.ts \
  src/lib/graph-stream/full-edit-document-job-session.test.ts \
  src/lib/components/graph-viewer/graph-viewer-render-effects.test.ts \
  src/lib/components/graph-viewer/graph-render-session.test.ts \
  src/lib/store/editor-workspace.test.ts \
  src/lib/store/editor-store.test.ts
pnpm test:integration
pnpm build:e2e
pnpm exec playwright test \
  test/e2e/multi-tab-sidecar.spec.ts \
  test/e2e/import-format-recognition.spec.ts \
  test/e2e/editor-core-real-chain.spec.ts
pnpm check:circular
```

完成前再执行完整 `pnpm test:unit`。若修改动态 import seam，额外执行 `pnpm build` 并确认没有 `[INEFFECTIVE_DYNAMIC_IMPORT]`。

最后运行 `git diff --numstat`。本重构应删除 global paths 并降低 `EditorCore.svelte` 的职责；若非测试生产代码净增长，必须说明新增 Module 带来的 locality/leverage，以及删除了哪些旧调用路径。
