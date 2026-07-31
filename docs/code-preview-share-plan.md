---
summary: "结构体代码生成、只读代码预览与分享链接持久化的实施方案。"
read_when:
  - 实现右侧只读代码预览模式
  - 将生成代码写入分享链接
  - 扩展分享协议以支持非 Treease 结构化语言
---

# 结构体生成代码预览与分享链接实施方案

## 1. 目标

将 JSON → 多语言结构体定义接入右侧编辑器，并提供独立的只读代码预览模式：

- 使用真实目标语言的 Monaco 语法高亮；
- 不让 Go、Rust、Java 等代码进入 Treease Core 的结构化解析链路；
- 生成结果可通过分享链接完整恢复；
- 分享链接恢复的是已经生成的代码，不重新调用生成 API。

当前生成入口、服务端 API 和底部交互栏已经存在。本方案主要覆盖右侧编辑器模式、分享协议和恢复流程。

## 2. 当前问题

当前 `showViewerTextPreview` 接收的是 `SupportedEditorLanguageId`，而右侧编辑器只支持：

```text
json / yaml / toml / javascript / python
```

生成结果目前通过临时映射显示：

```ts
python → python
其他语言 → javascript
```

这会导致 Go、Rust、Java 等代码没有正确高亮，并且可能被错误地当作结构化文档参与解析、图构建或同步。

当前分享协议也只允许右侧使用上述五种语言，并且没有保存 `RightEditorMode`，因此不能可靠地恢复代码预览状态。

## 3. 设计决策

### 3.1 增加独立的右侧模式

```ts
type RightEditorMode = 'structured' | 'code-preview';
```

两种模式的职责不同：

| 模式 | 编辑能力 | Monaco 语言 | Core 解析 | 图/比较操作 |
|---|---|---|---|---|
| `structured` | 现有行为 | Treease 支持的语言 | 参与 | 支持 |
| `code-preview` | 只读 | 真实生成语言 | 不参与 | 隐藏 |

不要只给现有 `SidecarEditor` 设置 `readOnly: true`。只读属性不能自动阻止 workspace 同步、Core 解析和图操作。

### 3.2 右侧编辑器按模式分离

建议由 `ViewportPanel` 统一管理模式，并提供两个内部渲染路径：

```text
ViewportPanel
 ├─ structured   → SidecarEditor
 └─ code-preview → ReadonlyCodePreviewEditor
```

`ReadonlyCodePreviewEditor` 只负责 Monaco model、语言加载、文本显示和只读配置。

### 3.3 生成后的交互

1. 用户从命令面板执行结构体生成；
2. 生成成功后自动切换：`structured → code-preview`；
3. 右侧强制显示 Text；
4. 右侧顶部显示目标语言和“返回结构化视图”入口；
5. 代码预览模式隐藏 Graph、Compare、Swap、结构化格式化等操作；
6. 返回结构化视图时退出代码预览，不重新生成代码。

当前会话中可以保存生成前的结构化右侧快照并恢复。分享链接只保证恢复分享时保存的模式和内容；如果分享链接本身是代码预览，不要求恢复分享前的本地临时快照。

## 4. 分享协议设计

### 4.1 版本策略

在 `@treease/share-protocol` 中增加 `text_snapshot` 的 `schemaVersion: 2`：

- 继续读取现有 `schemaVersion: 1` 链接；
- 新创建的链接使用 v2；
- 不修改旧 v1 payload 的语义；
- 服务端继续通过同一个 `shareResourceSchema` 校验资源。

v2 的 `rightMode` 显式记录右侧模式：

```json
{
  "type": "text_snapshot",
  "payload": {
    "schemaVersion": 2,
    "left": {
      "text": "{\"id\":1}",
      "languageId": "json"
    },
    "rightMode": "code-preview",
    "right": {
      "text": "type User struct { Id int `json:\"id\"` }",
      "languageId": "go"
    },
    "layout": {
      "viewMode": "text",
      "activePane": "right"
    },
    "interaction": {
      "treePath": [],
      "focus": null,
      "columnNavigator": { "activePath": [] }
    }
  }
}
```

### 4.2 语言协议

在 share protocol 中增加独立的 code-preview 语言枚举：

```text
typescript / go / rust / python / java / kotlin /
csharp / swift / dart / ruby / php
```

不要把这些语言直接加入 Treease 结构化语言枚举。`rightMode` 决定 `right.languageId` 使用哪一套语言约束：

- `structured`：只能使用现有 Treease 语言；
- `code-preview`：只能使用 code-preview 语言。

协议层应通过 Zod refine 或等价约束拒绝模式和语言不匹配的 payload。

### 4.3 布局约束

code-preview 资源必须满足：

```text
rightMode = code-preview
layout.viewMode = text
right != null
layout.activePane = right（推荐）
```

结构化资源继续支持 Graph/Text 两种视图。

### 4.4 交互状态恢复

`interaction` 现有结构主要描述结构化文档的树路径、图焦点和 column navigator。

恢复 `code-preview` 时：

- 恢复左侧主文档；
- 清理 compare 状态；
- 设置右侧模式为 `code-preview`；
- 调用 `showCodePreview(text, languageId)`；
- 强制设置 Text 视图；
- 不执行右侧结构化图操作、Column Navigator 恢复或 Core 解析。

恢复 `structured` v1/v2 时保持现有交互恢复逻辑。

## 5. Web API 与组件边界

### 5.1 `ViewportPanel` 对外 API

新增或调整为以下最小接口：

```ts
showCodePreview(text: string, languageId: CodePreviewLanguageId): Promise<void>;
getRightEditorMode(): RightEditorMode;
getCodePreview(): { text: string; languageId: CodePreviewLanguageId } | null;
setRightEditorMode(mode: RightEditorMode): Promise<void>;
```

现有 `showTextPreview` 保持结构化模式职责，不再接收 codegen 语言。

### 5.2 `ReadonlyCodePreviewEditor`

职责：

- 创建或复用 Monaco model；
- 加载目标语言的 Monaco language contribution；
- 设置真实 language ID；
- 设置 `readOnly: true`；
- 提供文本和 viewport 读取；
- 不注册 Treease semantic token provider；
- 不调用 Worker、WASM、Document Runtime 或 workspace commit。

动态 import 必须集中在 `*.runtime.ts` lazy boundary，避免同一生产模块同时存在静态和动态 import。完成后运行构建并检查 `[INEFFECTIVE_DYNAMIC_IMPORT]`。

### 5.3 语言加载

生成语言到 Monaco language ID 的映射应集中管理，不要在页面中散落字符串：

```text
TypeScript → typescript
go         → go
Rust       → rust
Python     → python
Java       → java
Kotlin     → kotlin
C#         → csharp
Swift      → swift
Dart       → dart
Ruby       → ruby
PHP        → php
```

如果某个 Monaco contribution 不可用，必须显式报错或显示未加载状态；不能再次静默降级到 JavaScript。Fallback 不是本方案的一部分。

## 6. 分享创建与恢复改动

### 6.1 创建分享

`apps/web/src/routes/editor/+page.svelte` 当前的 `createShareResource` 需要读取：

```text
left 文档
rightMode
right 文本
right languageId
layout
interaction
```

规则：

- `structured`：沿用现有右侧文档读取逻辑；
- `code-preview`：从 `getCodePreview()` 获取文本和真实目标语言；
- code-preview 分享强制 `viewMode: 'text'`；
- 不重新调用 `generateStruct`；
- 生成结果必须以 UTF-8 文本直接写入分享资源。

### 6.2 恢复分享

`apps/web/src/lib/share/share-restore.ts` 增加模式分支：

```text
v1 text_snapshot
  → 现有恢复逻辑

v2 structured
  → 现有结构化恢复逻辑

v2 code-preview
  → 恢复左侧文档
  → 清理 compare
  → 设置 code-preview
  → 加载真实 Monaco 语言
  → 显示只读代码
  → 跳过结构化交互恢复
```

恢复过程必须保持现有异步 freshness 约束，旧分享恢复结果不能覆盖用户当前文档。

## 7. Server 与存储影响

服务端当前分享接口保存并返回协议资源，新增 v2 后原则上不需要新增 API 路由或数据库字段。

实施者需要确认：

- `resource_type` 没有硬编码只允许两种资源类型的数据库约束；
- Server 使用更新后的 `@treease/share-protocol` schema 校验 v2；
- 分享 payload 的大小限制仍然适用；
- 不在日志中打印生成代码、用户 JSON 或完整分享 payload。

需要同步更新 `apps/server/AGENTS.md` 中过时的分享约束：分享可以携带用户明确生成并选择分享的 code-preview 内容，但不应由分享服务重新执行 AI/生成任务。

存储与过期策略继续遵循 `docs/contracts/file-storage.md`。本功能不新增 R2、Durable Object、Workflow 或 Agents SDK。

## 8. 实施顺序

### 阶段一：协议

- [ ] 在 `packages/share-protocol/src/index.ts` 增加 code-preview 语言 schema；
- [ ] 增加 `text_snapshot` v2 schema；
- [ ] 保留并测试 v1 兼容读取；
- [ ] 增加模式与语言一致性校验；
- [ ] 更新协议单元测试。

### 阶段二：只读编辑器

- [ ] 增加 `RightEditorMode` 类型及状态；
- [ ] 抽取 `ReadonlyCodePreviewEditor`；
- [ ] 注册 Go、Rust、Java 等 Monaco language contribution；
- [ ] 实现 codegen → Monaco language 映射；
- [ ] 确保 code-preview 不进入 Worker/WASM/Core/workspace；
- [ ] 在右侧工具栏增加模式状态和返回入口。

### 阶段三：生成流程

- [ ] 生成成功后进入 `code-preview`；
- [ ] 移除 `rightEditorLanguageForStruct` 的 JavaScript fallback；
- [ ] 生成失败时保持原有结构化模式；
- [ ] 处理重复生成、关闭底栏和切换文档时的状态清理。

### 阶段四：分享创建与恢复

- [ ] `createShareResource` 写入 v2 的 `rightMode`；
- [ ] code-preview 分享保存真实语言和完整代码；
- [ ] `restoreShareResource` 增加 code-preview 恢复分支；
- [ ] 恢复时跳过结构化图和 Column Navigator 交互；
- [ ] 覆盖 v1、v2 structured、v2 code-preview 三类链接。

### 阶段五：清理与文档

- [ ] 删除临时 JavaScript fallback；
- [ ] 删除仅为 fallback 保留的类型适配；
- [ ] 更新 `apps/server/AGENTS.md` 分享约束；
- [ ] 更新相关组件注释和本方案状态。

## 9. 测试要求

### 协议测试

- v1 text snapshot 仍可解析；
- v2 structured 可解析；
- v2 code-preview 可解析；
- code-preview 使用 `go`、`rust`、`java` 等语言可解析；
- structured 使用 `go` 被拒绝；
- code-preview 使用 `json` 被拒绝；
- code-preview 缺少 right 或使用 graph view 被拒绝；
- 未知字段和未知语言被拒绝。

### Web 单元测试

- 生成成功自动切换到 code-preview；
- 生成失败不改变现有模式；
- code-preview 为只读；
- code-preview 不调用结构化解析或图操作；
- 返回 structured 模式后恢复正确视图；
- 创建分享时保存 code-preview 文本、语言和模式；
- 三类分享资源恢复到预期模式。

### 手工验收

至少验证：

1. JSON 生成 Go，右侧显示 Go 高亮；
2. JSON 生成 Rust，右侧显示 Rust 高亮；
3. code-preview 中无法编辑文本；
4. Graph、Compare、Swap 等结构化操作不可用；
5. 创建分享链接并在新窗口打开，仍显示相同代码和语言高亮；
6. 分享链接恢复后不会触发生成 API；
7. 旧 v1 分享链接仍能正常打开；
8. 左侧 JSON 改变后，已分享的 code-preview 不被静默覆盖。

## 10. 验证命令

从仓库根目录执行最小相关检查：

```text
pnpm --dir apps/web check
pnpm --dir apps/web test:unit
cd apps/server && node --import tsx --test src/**/*.test.ts
pnpm build
pnpm check:circular
pnpm check:docs
```

如果改动了共享协议生成链路，还要按根目录 `AGENTS.md` 的协议验证要求执行对应生成和同步检查。

## 11. 完成标准

满足以下条件后可交付：

- 右侧 code-preview 使用真实目标语言高亮；
- code-preview 完全脱离结构化解析、图构建和 workspace commit；
- 模式切换有清晰入口，且不会丢失当前结构化文档；
- 新分享链接保存 `rightMode`、生成文本和目标语言；
- 新分享链接恢复后不重新生成、不触发 Core 解析；
- 旧 v1 分享链接保持可用；
- 协议、组件、恢复流程和服务端校验均有测试；
- 不引入 Agents SDK、Durable Object、Workflow 或新的生成 fallback。

## 12. 相关代码与契约

- `apps/web/src/routes/editor/+page.svelte`
- `apps/web/src/lib/components/ViewportPanel.svelte`
- `apps/web/src/lib/components/Editor/SidecarEditor.svelte`
- `apps/web/src/lib/share/share-resource.ts`
- `apps/web/src/lib/share/share-restore.ts`
- `packages/share-protocol/src/index.ts`
- `apps/server/src/services/share-service.ts`
- `apps/server/src/services/struct-generation-service.ts`
- `docs/contracts/file-storage.md`
- `docs/contracts/editor-data-flow.md`
- `ARCHITECTURE.md`
