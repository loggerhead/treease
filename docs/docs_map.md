# Treease docs map

This file is generated from `docs/**/*.md` and `docs/**/*.mdx` headings to help agents navigate the documentation tree.
Do not edit it by hand; run `pnpm docs:map:gen`.

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

## geo-checklist.md

- Route: /geo-checklist
- Headings:
  - H1: Treease GEO 提升 Checklist
  - H2: 1. 先统一目标：GEO 不是“操纵 AI 推荐”
  - H2: 2. 证据等级与使用方式
  - H2: 3. P0：测量和基线
  - H3: 3.1 建立固定 Prompt Set
  - H3: 3.2 指标定义
  - H3: 3.3 Treease 当前基线
  - H2: 4. P0：抓取、索引和生成式搜索访问
  - H3: 4.1 页面访问
  - H3: 4.2 robots 与 OpenAI crawler
  - H3: 4.3 可选的机器可读入口
  - H2: 5. P0：实体、信任和事实一致性
  - H3: 5.1 Treease 实体卡
  - H3: 5.2 跨站一致性
  - H3: 5.3 证据而不是口号
  - H2: 6. P0：让页面成为“可引用单元”
  - H3: 6.1 页面模板
  - H3: 6.2 Treease 教程页
  - H3: 6.3 结构化数据
  - H2: 7. P1：主题集群与站内关系图
  - H2: 8. P1：社区与第三方分发
  - H3: 8.1 Reddit
  - H3: 8.2 Hacker News
  - H3: 8.3 知乎
  - H3: 8.4 小红书
  - H3: 8.5 掘金
  - H3: 8.6 V2EX
  - H2: 9. P1：官方 AI 生态给出的可执行启示
  - H3: OpenAI
  - H3: Anthropic
  - H3: Meta AI
  - H3: xAI / Grok
  - H3: Google Research / DeepMind
  - H2: 10. P1：Treease 90 天执行清单
  - H3: 第 1～2 周：让所有事实可被读取
  - H3: 第 3～6 周：提高引用价值
  - H3: 第 7～10 周：建立第三方验证
  - H3: 第 11～12 周：复测并做小步迭代
  - H2: 11. 每周运营检查表
  - H2: 12. 明确不做的事情
  - H2: 13. 研究来源审计
  - H3: 用户指定社区
  - H3: 用户指定官方博客与研究站点
  - H3: 补充来源
  - H2: 14. 验收标准

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
  - H2: Product Boundaries
  - H2: Maintenance Rules
