# Treease docs map

This file is generated from `docs/**/*.md` and `docs/**/*.mdx` headings to help agents navigate the documentation tree.
Do not edit it by hand; run `pnpm docs:map:gen`.

## chrome-extension-json-graph-prd.md

- Route: /chrome-extension-json-graph-prd
- Headings:
  - H1: Treease Chrome 扩展：点击 JSON 后在侧边栏展示 Graph
  - H2: 1. 文档信息
  - H2: 2. 一句话定义
  - H2: 3. 背景与问题
  - H2: 4. 产品目标
  - H3: 4.1 MVP 目标
  - H3: 4.2 非目标
  - H2: 5. 核心用户流程
  - H2: 6. 用户故事
  - H3: US-01：点击 JSON 查看 Graph
  - H3: US-02：保持网页上下文
  - H3: US-03：本地处理敏感数据
  - H3: US-04：控制监听范围
  - H2: 7. 功能需求
  - H3: 7.1 点击监听
  - H3: 7.2 DOM 提取策略
  - H3: 7.3 JSON 检测
  - H3: 7.4 Side Panel
  - H3: 7.5 自动打开策略
  - H2: 8. 权限设计
  - H3: 8.1 初始权限建议
  - H3: 8.2 权限与审核解释
  - H2: 9. 隐私与数据处理
  - H3: 9.1 数据生命周期
  - H3: 9.2 默认行为
  - H3: 9.3 必须准备的合规材料
  - H2: 10. 技术架构
  - H2: 11. 失败和边界场景
  - H2: 12. 性能要求
  - H2: 13. MVP 验收标准
  - H3: P0
  - H3: P1
  - H3: P2
  - H2: 14. 成功指标
  - H2: 15. 主要风险与对策
  - H3: 风险一：自动打开 Side Panel 不稳定
  - H3: 风险二：&lt;allurls&gt; 引发用户和审核担忧
  - H3: 风险三：误读取敏感页面内容
  - H3: 风险四：点击监听造成性能影响
  - H3: 风险五：Treease Web 与扩展 Graph 代码重复
  - H2: 16. 发布与审核策略
  - H2: 17. 开放问题
  - H2: 18. 参考资料

## code-preview-share-plan.md

- Route: /code-preview-share-plan
- Headings:
  - H1: 结构体生成代码预览与分享链接实施方案
  - H2: 1. 目标
  - H2: 2. 当前问题
  - H2: 3. 设计决策
  - H3: 3.1 增加独立的右侧模式
  - H3: 3.2 右侧编辑器按模式分离
  - H3: 3.3 生成后的交互
  - H2: 4. 分享协议设计
  - H3: 4.1 版本策略
  - H3: 4.2 语言协议
  - H3: 4.3 布局约束
  - H3: 4.4 交互状态恢复
  - H2: 5. Web API 与组件边界
  - H3: 5.1 ViewportPanel 对外 API
  - H3: 5.2 ReadonlyCodePreviewEditor
  - H3: 5.3 语言加载
  - H2: 6. 分享创建与恢复改动
  - H3: 6.1 创建分享
  - H3: 6.2 恢复分享
  - H2: 7. Server 与存储影响
  - H2: 8. 实施顺序
  - H3: 阶段一：协议
  - H3: 阶段二：只读编辑器
  - H3: 阶段三：生成流程
  - H3: 阶段四：分享创建与恢复
  - H3: 阶段五：清理与文档
  - H2: 9. 测试要求
  - H3: 协议测试
  - H3: Web 单元测试
  - H3: 手工验收
  - H2: 10. 验证命令
  - H2: 11. 完成标准
  - H2: 12. 相关代码与契约

## contracts/api-boundary.md

- Route: /contracts/api-boundary
- Headings:
  - H1: API Boundary
  - H2: Ownership
  - H2: Contract rules
  - H2: Repository split gate

## contracts/bidirectional-edit.md

- Route: /contracts/bidirectional-edit
- Headings:
  - H1: Bidirectional Edit Contract
  - H2: Core Entities
  - H3: Editor Model
  - H3: Graph Interaction
  - H3: Graph Edit Planner
  - H3: Commit Transaction
  - H3: DocumentSnapshot
  - H2: Core Entity Relationships
  - H2: Bidirectional-Edit Constraints
  - H3: Editor → Graph
  - H3: Graph → Editor
  - H3: fallback
  - H2: Data Flow
  - H3: 1. Editor → Graph
  - H3: 2. Graph → Editor
  - H3: 3. Column Navigator Column Rail
  - H3: 4. Column Navigator column detail editor
  - H2: Column Navigator Entry-Point Constraints
  - H2: Checklist

## contracts/column-navigator.md

- Route: /contracts/column-navigator
- Headings:
  - H1: Column Navigator Contract
  - H2: Core Entities
  - H3: Column Navigator Active Path
  - H3: Column Navigator Surface
  - H3: Column
  - H3: Column Detail Editor
  - H3: Column Projection
  - H3: Local Draft
  - H3: Pending Commit
  - H2: Core Entity Relationships
  - H2: Column Navigator Constraints
  - H3: Subdomain role
  - H3: Column constraints
  - H3: Graph / content routing constraints
  - H3: Projection constraints
  - H3: Editing constraints
  - H3: Lifecycle constraints
  - H2: Data Flow
  - H3: 1. Main-graph click opens the Column Navigator
  - H3: 2. Drill down in a column
  - H3: 3. Edit in a column detail editor
  - H3: 4. External main-document refresh affects the Column Navigator
  - H3: 5. Consecutive commits on the same path
  - H2: Column Navigator Rules
  - H3: Path rules
  - H3: Cache rules
  - H2: Module Architecture
  - H3: Interaction-consistency rules
  - H2: Checklist

## contracts/desktop-workspace.md

- Route: /contracts/desktop-workspace
- Headings:
  - H1: Desktop Workspace Architecture Contract
  - H2: Shared Workspace and Platform Boundary
  - H2: Workspace and File Permissions
  - H2: Application Identity and Deep Links
  - H2: Authentication and Sessions
  - H2: Privacy and External Content
  - H2: Distribution and Updates

## contracts/document-runtime.md

- Route: /contracts/document-runtime
- Headings:
  - H1: Document Runtime Contract
  - H2: Authority
  - H2: Core Terms
  - H2: Invariants
  - H2: Seams

## contracts/editor-data-flow.md

- Route: /contracts/editor-data-flow
- Headings:
  - H1: Editor Data Flow Contract
  - H2: Core Entities
  - H3: Editor Model
  - H3: Commit Transaction
  - H3: Document Runtime
  - H3: DocumentSnapshot
  - H3: Workspace Store
  - H3: Workspace Mirror Text
  - H3: Active Document Context
  - H3: View Runtime
  - H3: View Runtime Operation Lifecycle
  - H2: Core Entity Relationships
  - H2: Core Constraints
  - H3: Text Authority
  - H3: Commit Authority
  - H3: Semantic Authority
  - H3: Binding Authority
  - H3: View Runtime operation lifecycle
  - H2: Product Scenario Data Flows
  - H3: 1. User directly edits the primary document
  - H3: 2. Programmatic whole-document replacement
  - H3: 3. Primary-document semantic read
  - H3: 4. Runtime result enters visible state
  - H3: 5. Blank / whitespace close
  - H3: 6. First mutation of a shared draft
  - H2: Primary-Document Data-Flow Checklist

## contracts/layout.md

- Route: /contracts/layout
- Headings:
  - H1: Layout Contract
  - H2: Inputs and Outputs
  - H3: Inputs
  - H3: Outputs
  - H2: Core Entities
  - H3: Topology
  - H3: Graph Node
  - H3: Graph Edge
  - H3: Table Presentation
  - H3: Layout Result
  - H2: Core Entity Relationships
  - H2: I. Node Classification
  - H3: Mapping Tree Structures to Graph Structures
  - H3: Independent-Node Rules
  - H3: Sequence Classification Rules
  - H3: Consistency Requirements for Classification
  - H2: II. Node Presentation
  - H3: Scalar
  - H3: Object
  - H3: Empty-Container Degradation
  - H3: Headerless Table
  - H3: Header Table
  - H3: Header Table Fallback value Column
  - H3: Virtual Table
  - H2: III. Graph Scene Projection (Web)
  - H3: Projection Rules
  - H3: Projection Invariants
  - H2: IV. Geometry Rules
  - H3: X-Axis Rules
  - H3: Y-Axis Rules
  - H3: Edge Rules
  - H2: V. Consistency Constraints
  - H3: Full-Build / Streaming Consistency
  - H3: Changed-Region Consistency
  - H3: Geometry Consistency
  - H2: VI. Explicit Errors
  - H2: Checklist

## contracts/share-workspace.md

- Route: /contracts/share-workspace
- Headings:
  - H1: Share Workspace Contract
  - H2: Scope
  - H2: Authority
  - H2: Lifecycle
  - H2: Share Restore
  - H2: First Draft Mutation
  - H3: Direct Editor input
  - H3: Command mutations
  - H2: Canonical Topology Promotion
  - H2: Persistence
  - H2: Concurrency and Cleanup
  - H2: Error Classes
  - H2: Verification Checklist

## contracts/stream-pipeline.md

- Route: /contracts/stream-pipeline
- Headings:
  - H1: Streaming Contract
  - H2: Core Entities
  - H3: Stream Input
  - H3: Streaming DocumentJob
  - H3: Stream Decoder / Builder
  - H3: Streaming Graph Projector
  - H3: Final Snapshot
  - H2: Core Entity Relationships
  - H2: Data Flow
  - H3: True streaming for same-language JSON
  - H3: Pseudo-streaming for non-JSON
  - H3: ApplyEdits
  - H2: Timing
  - H3: enable nest parse
  - H3: enable auto format
  - H2: Implementation Constraints
  - H3: True-streaming constraints
  - H3: Pseudo-streaming constraints
  - H3: Clear / parse-failed constraints
  - H3: Nested JSON constraints
  - H3: Auto-format constraints
  - H3: Root scalar replace constraint
  - H2: Checklist

## formats/csv.md

- Route: /formats/csv
- Headings:
  - H1: CSV
  - H2: Encode
  - H2: Decode
  - H2: Web import and export

## formats/javascript.md

- Route: /formats/javascript
- Headings:
  - H1: JavaScript Object
  - H2: Parse: object literal
  - H2: Parse: nested structures
  - H2: Roundtrip: preserve JavaScript-style data
  - H2: Convert to another format
  - H2: Notes

## formats/python.md

- Route: /formats/python
- Headings:
  - H1: Python Dict
  - H2: Parse: dict literal
  - H2: Parse: nested objects and arrays
  - H2: Roundtrip: preserve Python-style data
  - H2: Convert to another format
  - H2: Notes

## formats/toml.md

- Route: /formats/toml
- Headings:
  - H1: TOML
  - H2: Parse: Simple
  - H2: Parse: Deep paths
  - H2: Encode: Scalar
  - H2: Parse: inline table
  - H2: Parse: Array Table
  - H2: Parse: Array of Array Table
  - H2: Parse: Empty Table
  - H2: Roundtrip: inline table attribute
  - H2: Roundtrip: table section
  - H2: Roundtrip: array of tables
  - H2: Roundtrip: arrays and scalars
  - H2: Roundtrip: simple
  - H2: Roundtrip: deep paths
  - H2: Roundtrip: empty array
  - H2: Roundtrip: sample table
  - H2: Roundtrip: empty table
  - H2: Roundtrip: comments

## generated/core-registry-capabilities.md

- Route: /generated/core-registry-capabilities
- Headings:
  - H1: Core Registry Capabilities
  - H2: Build Options
  - H2: Operators
  - H2: Formats

## graph-stream-benchmark.md

- Route: /graph-stream-benchmark
- Headings:
  - H1: Graph Stream Benchmark
  - H2: Purpose
  - H2: Frozen Scope
  - H2: Commands
  - H2: Output Files
  - H2: Key Metrics
  - H2: Recommendation Rules
  - H2: Regression Thresholds
  - H2: Current Frozen Recommendation (v2)
  - H2: Current Runtime Chunk-Size Policy
  - H2: Risk Markers
  - H2: Remaining Boundaries

## references/supported-syntax-and-operators.md

- Route: /references/supported-syntax-and-operators
- Headings:
  - H1: Supported Syntax and Operators
  - H2: Purpose
  - H2: Scope
  - H2: Current Code Entry Points
  - H2: Expression Syntax Overview
  - H3: Comments and Whitespace
  - H3: Identifiers and Literals
  - H3: Grouping and Collections
  - H3: Path Traversal
  - H3: Variables
  - H2: Operator Capability Overview
  - H3: Composition and Control
  - H3: Assignment
  - H3: Arithmetic and Comparison
  - H3: Collections and Traversal
  - H3: Strings and Conversion
  - H3: Metadata and Paths
  - H3: Encoding and Decoding
  - H2: Responsibilities of the Hand-Written Reference and Generated Snapshot
  - H2: Maintenance Rules

## references/yaml-common-subset.md

- Route: /references/yaml-common-subset
- Headings:
  - H1: Common YAML Subset
  - H2: Purpose
  - H2: Common YAML Subset
  - H2: Outside the Common Subset
  - H2: Current Failure-Set Classification

## share-workspace-fork-design.md

- Route: /share-workspace-fork-design
- Headings:
  - H1: Share Workspace Fork Implementation Design
  - H2: Background
  - H2: Design Choices
  - H3: Page-level orchestration
  - H3: Pure topology transition
  - H3: Separate mutation seams
  - H3: Persistence after authority publication
  - H2: Migration Steps
  - H2: Verification Plan
  - H3: Unit
  - H3: Integration
  - H3: End-to-end
  - H2: Removal Gate

## tab-lifecycle-unification-plan.md

- Route: /tab-lifecycle-unification-plan
- Headings:
  - H1: /goal：统一编辑器 Tab 生命周期
  - H2: Browser 恢复数据的产品与隐私语义
  - H2: 目标
  - H2: 必须达成的架构结果
  - H2: 状态转换契约
  - H3: Create
  - H3: Activate
  - H3: Close inactive left tab
  - H3: Close active left tab with another left tab remaining
  - H3: Close last left tab
  - H2: 不可违反的边界
  - H2: 会话启动契约
  - H2: 实施顺序
  - H2: 验收标准
  - H2: 必须验证的行为
  - H3: Unit
  - H3: Integration
  - H3: End-to-end

## tab-scoped-document-operation-lifecycle-plan.md

- Route: /tab-scoped-document-operation-lifecycle-plan
- Headings:
  - H1: /goal：按 Tab 收敛编辑器文档操作生命周期
  - H2: 目标
  - H2: 产品语义
  - H3: Switch
  - H3: Reactivate
  - H3: Close
  - H3: Same-tab supersede
  - H3: Different tabs
  - H2: 状态所有权
  - H3: Workspace tab state
  - H3: Tab operation runtime
  - H3: Active projection
  - H2: 稳定文档操作目标
  - H3: Document freshness
  - H3: Visible freshness
  - H2: Commit Transaction 契约
  - H2: DocumentJob 与 Graph attachment 所有权
  - H2: Targeted whole-document replacement
  - H2: Format command 契约
  - H2: Editor interaction 投影
  - H2: Sidecar 边界
  - H2: 明确不做
  - H2: 实施顺序
  - H2: 验收标准
  - H2: 必须验证的行为
  - H3: Unit
  - H3: Integration
  - H3: End-to-end
  - H2: 验证命令

## tab-state-and-operation-lifecycle-handoff.md

- Route: /tab-state-and-operation-lifecycle-handoff
- Headings:
  - H1: Tab 状态与文档操作生命周期交接设计
  - H2: 背景
  - H2: 目标
  - H3: 非目标
  - H2: 关键数据流和设计决策
  - H3: 1. 状态按对象归属，而不是按组件归属
  - H3: 2. 文档语言保持单向数据流
  - H3: 3. 布局状态形成独立闭环
  - H3: 4. 每个异步操作捕获稳定文档目标
  - H3: 5. 分离 document freshness 与 visible freshness
  - H3: 6. 操作和取消所有权按 Tab 收敛
  - H3: 7. Whole-document replacement 只有一个目标化入口
  - H3: 8. 模块职责与依赖方向
  - H2: 状态规则与约束
  - H3: Authority 规则
  - H3: 状态建模规则
  - H3: 转换与异步规则
  - H3: 模块与清理规则
  - H2: 关键执行计划
  - H3: 阶段一：建立状态分类与失败反馈
  - H3: 阶段二：收敛 authority 与 active projection
  - H3: 阶段三：建立按 Tab 的 operation runtime
  - H3: 阶段四：统一文档替换与命令落地
  - H3: 阶段五：清理、契约更新与验收
  - H2: 完成判定

## user-stories.md

- Route: /user-stories
- Headings:
  - H1: User Stories
  - H2: Scope
  - H2: Product Overview
  - H2: Target Users
  - H2: Core User Journeys
  - H2: User Stories
  - H3: US-01 Import a File and Identify Its Format Automatically
  - H3: US-02 Locate and View JSON Blocks in Mixed Text
  - H3: US-03 Organize Text in the Editor
  - H3: US-04 Locate Fields in the Graph and Tree Path
  - H3: US-05 Preview Values by Hovering in the Editor
  - H3: US-06 Open the Column Navigator from the Graph
  - H3: US-07 Navigate Nested Paths in the Column Navigator
  - H3: US-08 Synchronize Changes Between the Graph and Editor
  - H3: US-09 Export to a Target Format
  - H3: US-10 Compare Two Pieces of Content
  - H3: US-11 Open a Reproducible Entry Point Through a URL Preset
  - H3: US-12 Adjust Personal Preferences
  - H3: US-13 See Progress While Processing Large Files
  - H3: US-14 Process Structured Input on the Command Line
  - H3: US-15 Continue the Current Journey After Login
  - H3: US-16 Transform Structured Data with AI Assistance
  - H2: Product Boundaries
  - H2: Maintenance Rules
