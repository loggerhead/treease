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
  - H3: 3. Subgraph-workspace graph pane
  - H3: 4. Subgraph-workspace content pane
  - H2: Subgraph-Workspace Entry-Point Constraints
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

## contracts/subgraph-workspace.md

- Route: /contracts/subgraph-workspace
- Headings:
  - H1: Subgraph Workspace Contract
  - H2: Core Entities
  - H3: Workspace Chain
  - H3: Workspace Pane
  - H3: Graph Pane
  - H3: Content Pane
  - H3: Workspace Projection
  - H3: Local Draft
  - H3: Pending Commit
  - H2: Core Entity Relationships
  - H2: Subgraph Workspace Constraints
  - H3: Subdomain role
  - H3: Pane constraints
  - H3: Graph / content routing constraints
  - H3: Projection constraints
  - H3: Editing constraints
  - H3: Lifecycle constraints
  - H2: Data Flow
  - H3: 1. Main-graph click opens the workspace
  - H3: 2. Drill down in a graph pane
  - H3: 3. Edit in a content pane
  - H3: 4. External main-document refresh affects the workspace
  - H3: 5. Consecutive commits on the same path
  - H2: Workspace-Specific Rules
  - H3: Path rules
  - H3: Cache rules
  - H2: Module Architecture
  - H3: Interaction-consistency rules
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

## primary-document-commit-transaction-plan.md

- Route: /primary-document-commit-transaction-plan
- Headings:
  - H1: 主文档单一 Commit Transaction 技术方案
  - H2: 1. 决策摘要
  - H2: 2. 回归 case 说明
  - H2: 3. 范围与非目标
  - H3: 3.1 范围
  - H3: 3.2 非目标
  - H2: 4. 现状中需要删除的多权威
  - H2: 5. 职责与依赖方向
  - H2: 6. 核心数据模型
  - H3: 6.1 Editor Write Receipt
  - H3: 6.2 Terminal Baseline
  - H3: 6.3 Commit Transaction
  - H3: 6.4 Terminal Outcome Binding
  - H2: 7. Commit Transaction 串行状态机
  - H3: 7.1 为什么增量输入关闭 close-time format
  - H3: 7.2 Pending delta 的增量合并
  - H2: 8. 各入口的数据流
  - H3: 8.1 Editor 连续输入
  - H3: 8.2 Graph / Subgraph Graph Pane
  - H3: 8.3 Planner replace
  - H3: 8.4 Whole replace / import
  - H3: 8.5 Subgraph Content Pane Pending Commit
  - H2: 9. Terminal Outcome 到 View Runtime
  - H3: 9.1 Workspace landing 顺序
  - H3: 9.2 主图
  - H3: 9.3 Subgraph Workspace
  - H2: 10. Freshness、冲突与错误
  - H3: 10.1 三层校验
  - H3: 10.2 Terminal 状态
  - H2: 11. 性能预算
  - H3: 11.1 单值 Editor / Graph 编辑
  - H3: 11.2 连续输入
  - H3: 11.3 Whole replace / import
  - H2: 12. 模块改造
  - H3: 12.1 Commit Transaction
  - H3: 12.2 EditorCore
  - H3: 12.3 Graph edit
  - H3: 12.4 Subgraph Workspace
  - H3: 12.5 Graph rendering
  - H2: 13. 迁移顺序
  - H3: Phase 1：先建立可观测的单一 outcome
  - H3: Phase 2：删除 Graph 第二个 job
  - H3: Phase 3：统一 Editor 写入和 transaction lane
  - H3: Phase 4：结构化 intent 与 Pending Commit
  - H3: Phase 5：删除旧抽象和兼容分支
  - H2: 14. 验收与证明
  - H3: 14.1 必过 E2E
  - H3: 14.2 单元测试
  - H3: 14.3 性能回归
  - H2: 15. 完成定义

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
  - H3: US-06 Click the Graph to Open a Local Workspace
  - H3: US-07 Open Nested Subgraph Workspaces in the Graph
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
