---
summary: "WASM language pack 拆分计划，定义按需加载 YAML 等格式能力的目标、模块边界、数据流和验收标准。"
read_when:
  - 任务涉及降低 Core WASM 首次加载体积或按需加载 YAML/TOML/Python/JavaScript/CSV 能力
  - 任务涉及拆分 packages/core 的语言能力、WASM 初始化、Document Runtime authority 或 Worker 语言包加载链路
---
# WASM Language Packs 拆分计划

## 目标

本计划的目标是把非首屏必须的格式能力从默认 Core WASM 首次加载路径中移出，优先以 YAML 为第一条落地语言包验证链路。

当前 `packages/core/Cargo.toml` 已有 `lite` feature，语义是只保留 JSON 并排除 YAML/TOML/Python/JavaScript/CSV。一次本地对照构建显示：

- full release wasm：3,417,645 bytes。
- lite release wasm：2,749,736 bytes。
- 差值约 667,909 bytes，覆盖 YAML/TOML/Python/JavaScript/CSV 及其 tree-sitter、decoder、encoder、registry、planner 相关代码。

最终目标不是依赖工具链自动把单体 WASM 切块，而是建立显式的 language pack 架构：

- 首屏只加载 Core base。
- JSON 保持默认可用。
- YAML 等语言能力在首次使用时由 Worker 按需加载。
- Document Runtime、snapshot、freshness、mainGraph、snapshot-bound read 的 authority 仍只在 Core base。

## 非目标

- 不把 Document Runtime 拆到语言包中。
- 不让 Worker 或 Web 状态层重新解释 snapshot、stale、diagnostics-only 或 mainGraph 语义。
- 不跨 WASM 模块共享 Rust 对象、指针、tree-sitter tree 或 `TreeStore` 借用。
- 不以 Binaryen 或 wasm-bindgen 的自动 code splitting 作为主方案。
- 不在第一阶段同时拆所有语言；YAML 是验证语言，其他格式跟随同一接口迁移。

## 模块划分

### Core base

Core base 是首屏加载的 WASM 模块，保留主文档 authority 和基础 JSON 能力。

必须保留在 Core base 的职责：

- `packages/core/src/document/runtime.rs` 中的 `DocumentRuntime`，包括 job handle、snapshot id、request seq、job、snapshot、latest snapshot。
- `packages/core/src/wasm_document.rs` 作为主文档 WASM 边界。
- `packages/core/src/document/protocol.rs` 作为 protocol 真源。
- `packages/core/src/wasm/runtime.rs` 中与全局 runtime 相关的状态，例如 `GLOBAL_STORE`、`REGISTRY_OWNER` 和 arena。
- JSON decode、format、streaming、graph build、query snapshot、hover subgraph projection。
- Language capability registry，用于记录当前已加载的语言包能力。

Core base 不直接内置 YAML/TOML/Python/JavaScript/CSV 的 decoder、encoder、tree-sitter grammar、query 和 graph value edit 规则。迁移窗口内可以保留兼容编译目标，但默认 Web 首屏构建必须走 base 入口。

### YAML language pack

YAML language pack 是按需加载的 WASM 模块，只提供 YAML 相关纯计算能力。

第一阶段 YAML pack 包含：

- `packages/core/src/formats/decoder_yaml.rs` 的 YAML decode 能力。
- `packages/core/src/formats/encoder_yaml.rs` 的 YAML encode/format 能力。
- YAML tree-sitter grammar 与 `packages/core/src/queries/yaml.scm`。
- YAML semantic token span 计算。
- YAML graph value edit 规则，对应 `packages/core/src/document/value_edit/yaml.rs`。
- YAML `LangSpec` 中和结构化路径、node type、query、扩展名相关的静态描述。

YAML pack 不保存文档状态，不分配 snapshot id，不提交 snapshot，不保留 latest snapshot，也不直接向 UI 输出 `DocumentEvent`。

### Web Worker language pack loader

Worker 负责加载与缓存 language pack JS/WASM glue，但不拥有语言语义。

Worker 侧新增一个 language pack loader，职责是：

- 根据语言名判断是否需要加载额外 pack。
- 缓存每个 pack 的初始化 Promise，避免并发重复加载。
- 把 pack 的导出能力注册给 Core base。
- 把加载失败转换成统一 Worker error 响应。

Worker 不判断某次分析应提交 authoritative snapshot 还是 diagnostics-only snapshot；这些判断仍由 Core base 完成。

## Core API 形态

Core base 需要新增一个内部 capability 抽象，用来替代分散的语言 `match` 分支。

概念接口如下：

```text
LanguageCapability {
  language_id
  aliases
  extensions
  decode(source, options) -> LanguageDecodeResult
  encode(document, options) -> LanguageEncodeResult
  semantic_tokens(source, options) -> SemanticTokenResult
  tree_path_support() -> TreePathLanguageSpec
  plan_graph_value_edit(request, analysis) -> GraphValueEditPlan
}
```

跨 WASM 模块调用时，不传 Rust 对象引用，只传可序列化值：

- source text。
- format options。
- graph edit request。
- JSON 兼容的 tree/value payload。
- diagnostics。
- semantic token `u32` 序列。
- document text edits。

Core base 收到 pack 结果后，将其转回 Core 内部 owned 数据，再进入现有 Document Runtime、snapshot 和 graph build 主链。

## 加载链路

首屏 JSON 路径：

```text
UI
  -> Worker
  -> Core base init
  -> Monaco shell ready
  -> JSON DocumentJob
  -> SnapshotReady / ParseFailed
```

首次 YAML 路径：

```text
UI 选择 yaml 或打开 .yaml/.yml
  -> Worker startDocumentJob(language = yaml)
  -> Core base ensureLanguageCapability(yaml)
  -> Worker load yaml pack
  -> yaml pack init
  -> Worker register yaml capability into Core base
  -> Core base resumes DocumentJob
  -> Core base delegates YAML decode/token/edit compute to yaml pack
  -> Core base commits snapshot
  -> Worker fan-out EventBatch
  -> UI render
```

后续 YAML 路径：

```text
UI
  -> Worker
  -> Core base
  -> cached yaml capability
  -> DocumentJob / snapshot-bound read / graph value edit
```

## 数据流变化

### AnalyzeSource

当前链路中，Core 单体内部按 language 分支选择 decoder、tree-sitter、semantic token 和 graph value edit 规则。

拆分后：

- Worker 仍只提交 `DocumentJob` 请求。
- Core base 创建 job、分配 request seq，并检查 language capability。
- 如果能力未加载，Core base 返回结构化缺失状态，Worker 加载 pack 后重试或继续该 job。
- YAML pack 返回 decode 与 analysis 所需的 owned 数据。
- Core base 生成 `AnalysisDelta`、`SnapshotReady` 或 `ParseFailed`。

### ApplyEdits

`ApplyEdits` 仍必须基于同 document 的 base snapshot。

拆分后：

- base snapshot 只保存在 Core base。
- Core base 校验 `documentKey`、`baseSnapshotId`、request seq 和 stale。
- 如需 YAML 结构感知增量或 value edit 规则，Core base 调用 YAML capability。
- 最终 source text、graph、diagnostics 和 snapshot 仍由 Core base 收口。

### snapshot-bound read

`querySnapshot`、hover subgraph projection、path span、path value 等读取不进入 language pack。

原因：

- 它们必须绑定 Core base 中的 snapshot id。
- 它们不能按 document key 回退到 latest snapshot。
- 它们不应在读取 API 内偷偷创建 snapshot。

语言包只能参与生成 snapshot 前的语言计算，不参与 snapshot 读取 authority。

### graph value edit

YAML 的 value edit 规则从静态 `planner_for_rule_kind` 迁移到 capability registry。

迁移后：

- Core base 根据 snapshot 中的 analysis language 找到 capability。
- 找不到时返回 `UnsupportedLanguage` 或明确的 pack-missing 错误。
- 找到时委托 language pack 生成 `GraphValueEditPlan`。
- Web 仍只消费 plan，不复制 YAML replacement 规则。

## 全局状态处理

### 必须留在 Core base 的状态

这些状态是主文档 authority，不能复制到语言包：

- `GLOBAL_DOCUMENT_RUNTIME`：job、snapshot、request seq、latest snapshot。
- `GLOBAL_STORE`：WASM runtime 共享 store。
- `REGISTRY_OWNER`：Core registry owner。
- `STORED_ANALYSES`：按 document key 缓存的分析结果。
- graph/tree/compare arena：WASM 输出 payload 的临时 owned buffer。

### 可以在 pack 内局部持有的状态

这些状态是纯计算缓存或只读静态表，可在每个 pack 内独立存在：

- tree-sitter grammar 和 query。
- semantic token query cache。
- YAML decoder/encoder 局部缓存。
- YAML graph value edit planner 静态对象。

### 需要迁移成 registry 的状态

现有语言能力分散在 `LangSpec`、decoder registry、format registry 和 value edit planner 中。拆分后应由 Core base 持有一份 language capability registry。

迁移原则：

- registry 只记录能力是否可用和如何调用。
- registry 不保存 document 级状态。
- pack 注册是幂等的。
- pack unload 不作为第一阶段目标。

## 落地阶段

### 阶段 0：基线与边界确认

目标：

- 保留现有 full 构建。
- 固化 full 与 lite 产物体积基线。
- 列出 YAML 能力涉及的 Rust 模块和测试覆盖面。

验收：

- 有脚本或 CI job 输出 full/lite wasm bytes。
- `docs/core/wasm-language-packs.md` 中的基线数字可复核。
- YAML 相关测试矩阵明确覆盖 decode、encode、semantic tokens、graph value edit、snapshot event。

### 阶段 1：Core base 内部 capability registry

目标：

- 在单 WASM 内先引入 language capability registry。
- JSON/YAML 仍编译在同一个模块里，但调用路径先从直接 `match` 改成 registry。
- Document Runtime 和 protocol 行为不变。

验收：

- JSON 和 YAML 的 `AnalyzeSource`、`ApplyEdits`、format、graph value edit 通过现有测试。
- `querySnapshot` 仍显式要求 snapshot id，不出现 latest fallback。
- Worker 侧 API 不需要感知 registry 细节。

### 阶段 2：base 构建排除 YAML 默认能力

目标：

- Web 首屏构建使用 Core base，只内置 JSON。
- YAML 在未加载时返回明确的 missing capability 状态。
- UI 能展示语言包加载中或加载失败状态。

验收：

- 首屏不下载 YAML pack。
- 打开 JSON 文档不触发 YAML pack 加载。
- 打开 YAML 文档会触发一次 YAML pack 加载。
- YAML pack 加载失败时不会提交伪 snapshot，不会清空已有 authoritative snapshot。

### 阶段 3：YAML pack 独立 WASM

目标：

- 编译独立 YAML pack WASM。
- Worker 能动态 import YAML pack。
- Core base 能通过稳定序列化 ABI 调用 YAML decode、encode、semantic tokens 和 graph value edit。

验收：

- 首次 YAML 分析完成后，Core base 提交 `SnapshotReady`，且 `mainGraph` 与 full 构建一致。
- invalid YAML 提交 `ParseFailed`，并清 graph，行为与 full 构建一致。
- YAML graph value edit 结果与 full 构建一致。
- 同一 YAML 文档连续编辑时，request seq 和 stale 行为不变。

### 阶段 4：Web 集成与性能验收

目标：

- Worker language pack loader 接入现有 runtime。
- Monaco 语言切换、文件导入、URL preset、format/minify/sort/export 都能按需触发 pack。
- 记录首屏 bundle/WASM 下载体积变化。

验收：

- JSON 首屏路径通过 Web 集成测试。
- YAML 首次加载路径通过 Web 集成测试。
- E2E 覆盖打开 YAML、编辑、图更新、format、graph value edit。
- 首屏 Core WASM 下载体积下降接近 full/lite 差值中可归因的非 JSON 部分。

### 阶段 5：推广到其他格式

目标：

- TOML、Python、JavaScript、CSV 复用同一 capability registry 与 Worker loader。
- 不为每种语言发明新的加载协议。

验收：

- 每个新增 pack 都只新增语言实现与注册代码。
- Worker loader 不出现 per-language 特判。
- Core base 中的 language dispatch 不回退到大型 `match` 分支。

## 验收标准

### 功能验收

- JSON 首屏无需加载任何非 JSON language pack。
- YAML 在首次使用时按需加载，加载完成后完整支持 decode、format、semantic tokens、graph build、graph value edit。
- snapshot id、request seq、latest snapshot 只由 Core base 分配和维护。
- `SnapshotReady`、`ParseFailed`、`SnapshotNotReady` 语义与拆分前一致。
- Worker 不二次判断 authoritative、diagnostics-only、stale 或 mainGraph。

### 性能验收

- Core base WASM bytes 小于当前 full WASM。
- JSON 首屏不下载 YAML pack。
- YAML pack 只在以下触发点加载：语言切换到 YAML、打开 `.yaml`/`.yml`、导入源或目标需要 YAML、执行 `from_yaml`/`to_yaml` 相关能力。
- YAML pack 初始化结果缓存，连续 YAML 操作不重复下载或重复初始化。

### 架构验收

- `packages/core/src/document/protocol.rs` 仍是协议真源。
- `packages/core/src/wasm_document.rs` 仍是主文档 WASM 边界。
- Web 不直接调用 language pack 绕过 Worker。
- Worker 不保存文档语义状态。
- 跨 WASM 模块只传 owned/serialized 数据，不传 Rust 指针、借用或模块内部 handle。

### 测试验收

- Core：覆盖 registry dispatch、missing capability、YAML pack compute 结果、snapshot commit 行为。
- Web integration：覆盖 JSON 不加载 pack、YAML 首次加载、加载失败、加载后事件序列。
- E2E：覆盖用户打开 YAML、编辑文本、查看 graph、执行 format、执行 graph value edit。
- 文档：运行 `node scripts/check-docs.mjs` 通过。

## 风险与缓解

- 跨 WASM 数据复制增加 YAML 首次分析成本。缓解方式是只在语言能力边界复制，并缓存 pack 初始化结果。
- full 构建与 base/pack 构建可能行为漂移。缓解方式是保留同一套 fixture 和 golden tests，对 full 与 pack 路径做同输入同输出验证。
- registry 迁移期间容易出现并行 dispatch。缓解方式是先在单 WASM 内完成 registry 改造，再做物理拆分。
- 语言包加载失败可能污染当前 UI 状态。缓解方式是 Core base 返回结构化错误，不提交伪 snapshot，Web 只展示错误状态。
- pack ABI 过早暴露内部模型会阻碍演进。缓解方式是 ABI 使用稳定 owned payload，并由 Core base 负责转换成内部类型。
