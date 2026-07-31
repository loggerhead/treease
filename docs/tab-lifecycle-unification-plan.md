---
summary: "Executable /goal directive for unifying editor tab topology and Monaco model lifetime."
read_when:
  - Refactoring editor tab creation, activation, closing, or Monaco model lifetime
  - Changing workspace session restoration or tab state ownership
---

# /goal：统一编辑器 Tab 生命周期

## Browser 恢复数据的产品与隐私语义

浏览器端恢复仅保存当前 workspace 的本地草稿（Tab 名称、语言、文本和当前 Tab），存放在此浏览器配置文件的 IndexedDB `treease-workspace` 中；它不会上传、同步到账号或恢复文件系统访问授权。用户可通过编辑器重置本地数据清除它。Desktop 仍由 `WorkspaceHost` 恢复本地草稿；共享编辑器代码不直接访问 IndexedDB 或 Tauri。

## 目标

将左侧编辑器 Tab 的拓扑状态和 Monaco model 生命周期收敛为一条可证明的转换路径，消除 `TabManager`、`EditorCore` 与 `EditorWorkspaceState` 之间的重复所有权和时序竞态。

完成后，任意时刻必须满足：

```text
顶部 Tab 列表、active/primary Tab、Monaco 当前 model、editorIO、图与文档运行时
都指向同一个 documentKey。
```

关闭最后一个左侧 Tab 的正式产品语义为：

```text
close last left tab → 创建并激活一个新的空白 primary document
```

该新文档必须拥有新的 `tabId` 和 `documentKey`，文本为空；它不是 fallback，不得复用示例文本或被关闭文档的内容。

## 必须达成的架构结果

1. `EditorWorkspaceState`（由 `ActiveDocumentAuthority` 持有）是 Tab 拓扑的唯一事实来源，负责：
   - 左侧 Tab 的身份、顺序、名称和 active/primary 选择；
   - 左右 pane 的归属及 sidecar 身份；
   - revision、snapshot binding、文本镜像、dirty 元数据和 UI-local 状态。

2. Monaco 资源必须由 `EditorCore` 内部的私有 runtime adapter 管理。该 adapter 只负责：
   - `Map<tabId, ITextModel>`；
   - 从 workspace 文本镜像创建或取得 model；
   - 同步执行 `editor.setModel(model)`；
   - 拓扑转换完成后销毁已移除的 model。

   它不得拥有 Tab 顺序、Tab summary、`activeTabId`、名称或 Tab 选择策略。

3. 删除或彻底收缩 `TabManager.svelte`。不得保留一个同时维护第二份 Tab 列表、active id 或同步 facade 的兼容层。

4. TopBar、键盘命令和 host 命令只能调用 `EditorCore` 的 Tab 命令；TopBar 的列表和 active id 必须来自 `editorWorkspace` 的派生 selector。删除 `bind:tabSummaries`、`bind:activeTabId` 和 `syncTabBindings()` 作为状态转移机制的用法。

5. `workspaceCoordinator` 只能单向适配旧的 view-local store，不能发起 Tab 命令、修改拓扑或把值写回 workspace。

## 状态转换契约

在 `editor-workspace.ts` 附近建立一个纯拓扑转换模块。该模块只能输入不可变 workspace 状态和命令参数，输出封闭的转换结果；不得调用 Svelte、Monaco、worker 或 Document Runtime。

结果类型必须能穷举关闭后的合法结果，不得用 `null`、可选 fallback 参数、空字符串或哨兵 Tab id 表示“没有下一个 Tab”。可采用等价于以下语义的类型：

```ts
type TabTopologyEffect =
  | { kind: 'activate-existing'; tabId: string; disposeTabId?: string }
  | { kind: 'activate-new-blank'; tabId: string; documentKey: string; disposeTabId: string };

type TabTopologyTransition = {
  workspace: EditorWorkspaceState;
  effect: TabTopologyEffect;
};
```

必须实现以下命令：

### Create

- 在一个拓扑转换中生成新左侧 Tab 的 `tabId`、`documentKey`、语言、初始文本和镜像元数据；
- 将其加入拓扑并激活；
- 同步创建并选择 Monaco model；
- 发布 workspace；
- 再启动该文档已有的 whole-document commit 路径。

### Activate

- 缺失 id 或 sidecar id 必须被纯转换拒绝；
- 一次性更新 active/primary/left-pane id 和角色；
- 同步选择目标 model；若 model 不在内存中，只能从 workspace 文本镜像创建；
- 发布 workspace 后，按现有 document lifecycle 恢复或启动该文档工作，并以 `documentKey + revision` 做 freshness 约束。

### Close inactive left tab

- 只从拓扑移除请求的左侧 Tab；
- active Tab 和其 model 不得改变；
- 发布 workspace；
- 最后再销毁被移除的 resident model。

### Close active left tab with another left tab remaining

- 先在纯转换中根据剩余顺序计算 successor；
- 先同步 `editor.setModel(successorModel)`；
- 再发布不含已关闭 Tab、且 successor 为 primary/active 的 workspace；
- 最后销毁已关闭 model；
- 已关闭文档的异步工作必须通过既有 operation lifecycle 失效并清理，不得写入 successor 的可见状态。

### Close last left tab

- 在命令边界生成新空白 Tab 的 `tabId` 和 `documentKey`；
- 一次拓扑转换中移除旧 Tab、插入新 Tab，并令其成为唯一 primary/active 左侧 Tab；
- 保留右侧 sidecar；只保留仍属于 retained documents 的 snapshot binding；
- 创建并同步选择空 model；
- 发布 workspace；
- 销毁旧 model；
- 通过正常 `Commit Transaction` 路径提交空文本，使空白/clear 语义由 Document Runtime 产生。

任何路径都不得恢复示例、复用旧文本、静默保留已关闭 Tab，或在旧 model dispose 后才决定 successor。

## 不可违反的边界

- 不新增第二个 workspace/reducer store；`ActiveDocumentAuthority` 仍是 Web 端 active document 的唯一 authority。
- `Editor Model` 仍是 active draft text 的编辑来源；`EditorWorkspaceTab.sourceText` 只是 inactive/unmounted model、会话持久化和可见绑定所需的镜像，不得成为竞争性文本 authority。
- Tab 生命周期代码不得创建 snapshot、解释 parse result 或直接写 graph state；语义仍归 Document Runtime。
- 所有 Web 异步 UI 工作必须使用现有 `FreshnessScope` / View Runtime operation lifecycle；已关闭或非 active 文档的结果不得落入可见 UI。
- 左侧 Tab 与右侧 sidecar 必须保持不同生命周期；关闭左侧 Tab 不得关闭、激活或把 sidecar 序列化进左侧 `tabOrder`。
- Desktop 和 Browser 继续共享 Web workspace；会话 I/O 只能通过 `WorkspaceHost`，共享编辑器代码不得直接调用 Tauri 或 IndexedDB。

## 会话启动契约

会话恢复必须是 bootstrap 阶段，而不是与用户交互竞争的编辑器命令：

1. `WorkspaceHost.loadSession()` 只提供 host-owned persisted data；
2. 在编辑器接受任何 Tab 命令前，校验并转换出完整 workspace topology；
3. 从该 topology 构造并选择初始 Monaco model，然后发布 ready authority state；
4. 只有在此之后才启用 TopBar、键盘 Tab 命令和 session-save subscription。

Desktop 继续只恢复本地草稿。若保留 Browser session persistence，必须单独记录产品与隐私语义，不得作为本重构的隐式副作用引入。

## 实施顺序

1. 在 `apps/web/src/lib/store/editor-workspace.test.ts` 为 create、activate、close inactive、close active、close-last-to-blank 增加纯转换测试，覆盖 sidecar 保留和 snapshot binding 清理。
2. 将 `closeWorkspaceTab(..., fallback?)` 改为封闭的 close transition；删除 fallback 参数，以及“没有 fallback 时拒绝关闭最后 Tab”的旧测试和分支。
3. 在 editor component boundary 引入私有 Monaco resource adapter，迁移 model 创建、查找、激活、document-key 绑定和销毁逻辑。
4. 将 `EditorCore.svelte` 的 `addTab`、`activateTab`、`closeTab` 改为上述 canonical command sequence；明确保证 successor model 在旧 model dispose 前已安装。
5. 删除重复的 Tab summary 和本地 active-tab 状态，改由 `editorWorkspace` 派生 selector 驱动 header。
6. 让会话恢复先构造 workspace，再令 editor 进入 command-ready 状态；host 访问继续隔离在 `workspace-host`。
7. 删除过时的 `TabManager` API、兼容路径、双写同步 helper 和只保护旧实现的测试。
8. 在唯一的 Tab 命令边界添加生命周期不变量注释：必须先安装选中的 model，再销毁移除的 model，否则 `editorIO` 可能指向已释放文档，而 workspace 已渲染另一个 active Tab。

## 验收标准

只有同时满足以下条件才算完成：

- 只有一个模块定义左侧 Tab 拓扑和 active 选择；
- 只有一个私有 runtime 管理 Monaco model 资源生命周期；
- 没有组件维护第二份可变 Tab 列表或 active id；
- 没有 close path 接受或实现 fallback；
- 关闭最后一个左侧 Tab 必然产生全新空白 primary document；
- 所有可见异步结果都绑定到产生它们的 document context；
- 旧的重复状态、同步 helper、兼容 API 和过时测试均已删除。

## 必须验证的行为

### Unit

- active 和 primary id 始终指向现存左侧 Tab；
- close transition 只能返回定义的两种 effect；
- close-last 恰好产生一个空文本、全新 id、全新 document key 的 primary Tab；
- 每次左侧转换后 sidecar 仍存在且不在左侧 `tabOrder`；
- 被移除 document key 的 snapshot binding 不再可达，保留文档的 binding 不受影响。

### Integration

- 关闭 active Tab 时，successor model 在旧 model dispose 前已安装；`editorIO.getModel()` 与 authority active document 的 `documentKey` 相同；
- 关闭后编辑只推进 successor document 的 `Commit Transaction`；
- 已关闭文档的 pending job 不能改变可见 graph、diagnostics、snapshot binding 或 active text；
- 空白 last-tab replacement 经过 Document Runtime 的 authoritative blank/clear 行为，而不是只改变 UI。

### End-to-end

- 重现录屏路径：加载内容、创建 Tab、立即关闭 active Tab；header、source editor、active workspace tab 和 graph 全部指向同一剩余文档；
- 关闭 inactive Tab 不改变当前 editor model；
- sidecar 打开时关闭最后一个左侧 Tab，得到新的空白 primary，sidecar 保持不变；
- 恢复持久化多 Tab session 时，恢复完成前不接受 Tab 命令，恢复后的 header、editor 和 graph 保持一致。

执行最小相关 Web 检查，然后执行 `pnpm check:circular`。本目标不应引入 Core protocol 或 WASM 变更。
