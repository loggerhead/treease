---
summary: "Generated heading map for Treease docs pages"
read_when: "Finding which docs page covers a topic before reading the page"
title: "Docs map"
---

# Treease docs map

This file is generated from `docs/**/*.md` and `docs/**/*.mdx` headings to help agents navigate the documentation tree.
Do not edit it by hand; run `pnpm docs:map:gen`.

## CODING.md

- Route: /CODING
- Headings:
  - H1: Treease 编码协作规范
  - H2: 适用范围
  - H2: 目录边界
  - H2: 单一职责
  - H2: 修改优先级
  - H2: 禁止补丁式处理
  - H2: 可读性规则
  - H2: 所有权与生命周期规则
  - H2: 文档规则
  - H2: 临时脚本规则
  - H2: 禁止事项

## adr/0001-workspace-host-for-desktop.md

- Route: /adr/0001-workspace-host-for-desktop
- Headings:
  - H1: Workspace Host 作为桌面工作区的平台边界
  - H2: Considered Options

## adr/0002-vscode-style-tab-lifecycle.md

- Route: /adr/0002-vscode-style-tab-lifecycle
- Headings:
  - H1: VS Code 风格的 Workspace Tab 生命周期
  - H2: Considered Options

## adr/0003-least-privilege-file-access.md

- Route: /adr/0003-least-privilege-file-access
- Headings:
  - H1: 仅授权用户显式交付的文件
  - H2: Considered Options

## adr/0004-desktop-authentication-boundary.md

- Route: /adr/0004-desktop-authentication-boundary
- Headings:
  - H1: 系统浏览器登录与系统凭证库会话
  - H2: Considered Options

## adr/0005-no-cost-desktop-release.md

- Route: /adr/0005-no-cost-desktop-release
- Headings:
  - H1: 首版使用已签名的 Tauri 更新包
  - H2: Considered Options

## adr/0006-desktop-application-identity.md

- Route: /adr/0006-desktop-application-identity
- Headings:
  - H1: 桌面应用的稳定身份

## adr/0007-default-desktop-analytics.md

- Route: /adr/0007-default-desktop-analytics
- Headings:
  - H1: 默认启用最小化桌面分析
  - H2: Considered Options

## adr/0008-desktop-external-content-boundary.md

- Route: /adr/0008-desktop-external-content-boundary
- Headings:
  - H1: 桌面端的外部内容边界

## adr/0009-desktop-deep-link-scope.md

- Route: /adr/0009-desktop-deep-link-scope
- Headings:
  - H1: 桌面深链只打开编辑器工作区

## cli/index.md

- Route: /cli
- Headings:
  - H1: CLI
  - H2: Common Tasks
  - H2: Local Graph Page
  - H2: Machine-Readable Outputs
  - H2: Error Codes

## core/index.md

- Route: /core
- Headings:
  - H1: Core
  - H2: Domain Rules
  - H2: What This Domain Covers
  - H2: Read By Topic
  - H2: Stable Entry Points
  - H2: Relation To Other Domains

## core/wasm-language-packs.md

- Route: /core/wasm-language-packs
- Headings:
  - H1: WASM Language Packs 拆分计划
  - H2: 目标
  - H2: 非目标
  - H2: 模块划分
  - H3: Core base
  - H3: YAML language pack
  - H3: Web Worker language pack loader
  - H2: Core API 形态
  - H2: 加载链路
  - H2: 数据流变化
  - H3: AnalyzeSource
  - H3: ApplyEdits
  - H3: snapshot-bound read
  - H3: graph value edit
  - H2: 全局状态处理
  - H3: 必须留在 Core base 的状态
  - H3: 可以在 pack 内局部持有的状态
  - H3: 需要迁移成 registry 的状态
  - H2: 落地阶段
  - H3: 阶段 0：基线与边界确认
  - H3: 阶段 1：Core base 内部 capability registry
  - H3: 阶段 2：base 构建排除 YAML 默认能力
  - H3: 阶段 3：YAML pack 独立 WASM
  - H3: 阶段 4：Web 集成与性能验收
  - H3: 阶段 5：推广到其他格式
  - H2: 验收标准
  - H3: 功能验收
  - H3: 性能验收
  - H3: 架构验收
  - H3: 测试验收
  - H2: 风险与缓解

## desktop/implementation.md

- Route: /desktop/implementation
- Headings:
  - H1: Desktop Workspace 实施方案
  - H2: 目标与非目标
  - H2: 模块边界
  - H2: 实施切片
  - H3: 1. 桌面壳与共享入口
  - H3: 2. 多 Tab 文件生命周期
  - H3: 3. 会话与系统交互
  - H3: 4. 登录、云能力与网络边界
  - H3: 5. 更新、发布与测试
  - H2: 发布前检查

## desktop/index.md

- Route: /desktop
- Headings:
  - H1: Desktop
  - H2: 已确定决策
  - H2: 相关决策记录

## formats/csv.md

- Route: /formats/csv
- Headings:
  - H1: CSV
  - H2: Encode
  - H2: Decode
  - H2: Web import and export

## formats/index.md

- Route: /formats
- Headings:
  - H1: Formats
  - H2: Read By Need
  - H2: Scope

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

## generated/index.md

- Route: /generated
- Headings:
  - H1: Generated
  - H2: Generated Assets
  - H2: Rules
  - H2: Related Domains

## index.md

- Route: /
- Headings:
  - H1: Treease Docs
  - H2: What Treease Covers
  - H2: Start Here
  - H2: Theme Indexes
  - H2: Reading Paths
  - H2: Verification

## operators/add.md

- Route: /operators/add
- Headings:
  - H1: Add
  - H2: Concatenate arrays
  - H2: Concatenate to existing array
  - H2: Concatenate null to array
  - H2: Append to existing array
  - H2: Prepend to existing array
  - H2: Add new object to array
  - H2: Relative append
  - H2: String concatenation
  - H2: Number addition - float
  - H2: Number addition - int
  - H2: Increment numbers
  - H2: Date addition
  - H2: Add to null
  - H2: Add maps to shallow merge
  - H2: Custom types: that are really strings
  - H2: Custom types: that are really numbers

## operators/alternative-default-value.md

- Route: /operators/alternative-default-value
- Headings:
  - H1: Alternative (Default value)
  - H2: LHS is defined
  - H2: LHS is not defined
  - H2: LHS is null
  - H2: LHS is false
  - H2: RHS is an expression
  - H2: Update or create - entity exists
  - H2: Update or create - entity does not exist

## operators/array-to-map.md

- Route: /operators/array-to-map
- Headings:
  - H1: Array to Map
  - H2: Simple example

## operators/assign-update.md

- Route: /operators/assign-update
- Headings:
  - H1: Assign (Update)
  - H3: plain form: =
  - H3: relative form: |=
  - H3: Flags
  - H2: Create yaml file
  - H2: Update node to be the child value
  - H2: Double elements in an array
  - H2: Update node to be the sibling value
  - H2: Updated multiple paths
  - H2: Update string value
  - H2: Update string value via |=
  - H2: Update deeply selected results
  - H2: Update array values
  - H2: Update empty object
  - H2: Update node value that has an anchor
  - H2: Update empty object and array
  - H2: Custom types are maintained by default
  - H2: Custom types: clobber

## operators/boolean-operators.md

- Route: /operators/boolean-operators
- Headings:
  - H1: Boolean Operators
  - H2: Related Operators
  - H2: or example
  - H2: "yes" and "no" are strings
  - H2: and example
  - H2: Matching nodes with select, equals and or
  - H2: any returns true if any boolean in a given array is true
  - H2: any returns false for an empty array
  - H2: anyc returns true if any element in the array is true for the given condition.
  - H2: all returns true if all booleans in a given array are true
  - H2: all returns true for an empty array
  - H2: allc returns true if all elements in the array are true for the given condition.
  - H2: Not true is false
  - H2: Not false is true
  - H2: String values considered to be true
  - H2: Empty string value considered to be true
  - H2: Numbers are considered to be true
  - H2: Zero is considered to be true
  - H2: Null is considered to be false

## operators/collect-into-array.md

- Route: /operators/collect-into-array
- Headings:
  - H1: Collect into Array
  - H2: Collect empty
  - H2: Collect single
  - H2: Collect many

## operators/contains.md

- Route: /operators/contains
- Headings:
  - H1: Contains
  - H2: Array contains array
  - H2: Array has a subset array
  - H2: Object included in array
  - H2: Object not included in array
  - H2: String contains substring
  - H2: String equals string

## operators/create-collect-into-object.md

- Route: /operators/create-collect-into-object
- Headings:
  - H1: Create, Collect into Object
  - H2: Collect empty object
  - H2: Wrap (prefix) existing object
  - H2: Using splat to create multiple objects
  - H2: Working with multiple documents
  - H2: Creating yaml from scratch
  - H2: Creating yaml from scratch with multiple objects

## operators/delete.md

- Route: /operators/delete
- Headings:
  - H1: Delete
  - H2: Delete entry in map
  - H2: Delete nested entry in map
  - H2: Delete entry in array
  - H2: Delete nested entry in array
  - H2: Delete no matches
  - H2: Delete matching entries
  - H2: Recursively delete matching keys

## operators/divide.md

- Route: /operators/divide
- Headings:
  - H1: Divide
  - H2: String split
  - H2: Number division
  - H2: Number division by zero

## operators/encode-decode.md

- Route: /operators/encode-decode
- Headings:
  - H1: Encoder / Decoder
  - H2: Encode value as json string
  - H2: Encode value as json string, on one line
  - H2: Encode value as json string, on one line shorthand
  - H2: Decode a json encoded string
  - H2: Decode csv encoded string
  - H2: Encode value as yaml string
  - H2: Encode value as yaml string, with custom indentation
  - H2: Decode a yaml encoded string
  - H2: Update a multiline encoded yaml string
  - H2: Update a single line encoded yaml string
  - H2: Encode array of scalars as csv string
  - H2: Encode array of arrays as csv string
  - H2: Encode a string to base64
  - H2: Encode a yaml document to base64
  - H2: Decode a base64 encoded string
  - H2: Decode a base64 encoded yaml document

## operators/entries.md

- Route: /operators/entries
- Headings:
  - H1: Entries
  - H2: toentries Map
  - H2: toentries Array
  - H2: toentries null
  - H2: fromentries map
  - H2: fromentries with numeric key indices
  - H2: Use withentries to update keys
  - H2: Use withentries to update keys recursively
  - H2: Custom sort map keys
  - H2: Use withentries to filter the map

## operators/equals.md

- Route: /operators/equals
- Headings:
  - H1: Equals / Not Equals
  - H2: Related Operators
  - H2: Match string
  - H2: Don't match string
  - H2: Match number
  - H2: Don't match number
  - H2: Match nulls
  - H2: Non existent key doesn't equal a value
  - H2: Two non existent keys are equal

## operators/filter.md

- Route: /operators/filter
- Headings:
  - H1: Filter
  - H2: Filter array
  - H2: Filter map values

## operators/first.md

- Route: /operators/first
- Headings:
  - H1: First
  - H2: First matching element from array
  - H2: First matching element from array with multiple matches
  - H2: First matching element from array with numeric condition
  - H2: First matching element from array with boolean condition
  - H2: First matching element from array with null values
  - H2: First matching element from array with complex condition
  - H2: First matching element from map
  - H2: First matching element from map with numeric condition
  - H2: First matching element from nested structure
  - H2: First matching element with no matches
  - H2: First matching element from empty array
  - H2: First matching element from scalar node
  - H2: First matching element from null node
  - H2: First matching element with string condition
  - H2: First matching element with length condition
  - H2: First matching element from array of strings
  - H2: First matching element from array of numbers
  - H2: First element with no filter from array
  - H2: First element with no filter from array of maps

## operators/flatten.md

- Route: /operators/flatten
- Headings:
  - H1: Flatten
  - H2: Flatten
  - H2: Flatten with depth of one
  - H2: Flatten empty array
  - H2: Flatten array of objects

## operators/group-by.md

- Route: /operators/group-by
- Headings:
  - H1: Group By
  - H2: Group by field
  - H2: Group by field, with nulls

## operators/has.md

- Route: /operators/has
- Headings:
  - H1: Has
  - H2: Has map key
  - H2: Select, checking for existence of deep paths
  - H2: Has array index

## operators/index.md

- Route: /operators
- Headings:
  - H1: Operators
  - H2: Read By Need
  - H2: Scope

## operators/keys.md

- Route: /operators/keys
- Headings:
  - H1: Keys
  - H2: Map keys
  - H2: Array keys
  - H2: Retrieve array key
  - H2: Retrieve map key
  - H2: No key
  - H2: Update map key
  - H2: Check node is a key

## operators/kind.md

- Route: /operators/kind
- Headings:
  - H1: Kind
  - H2: Get kind
  - H2: Get kind, ignores custom tags

## operators/length.md

- Route: /operators/length
- Headings:
  - H1: Length
  - H2: String length
  - H2: null length
  - H2: Map length
  - H2: Array length

## operators/map.md

- Route: /operators/map
- Headings:
  - H1: Map
  - H2: Map array
  - H2: Map object values

## operators/min-max.md

- Route: /operators/min-max
- Headings:
  - H1: Min / Max
  - H2: Min
  - H3: Minimum int
  - H3: Minimum string
  - H3: Minimum of empty
  - H2: Max
  - H3: Maximum int
  - H3: Maximum string
  - H3: Maximum of empty

## operators/modulo.md

- Route: /operators/modulo
- Headings:
  - H1: Modulo
  - H2: Number modulo - int
  - H2: Number modulo - float
  - H2: Number modulo - int by zero
  - H2: Number modulo - float by zero

## operators/multiply-merge.md

- Route: /operators/multiply-merge
- Headings:
  - H1: Multiply (Merge)
  - H2: Objects and arrays - merging
  - H3: Merge Flags
  - H1: Merging complex arrays together by a key field
  - H2: Multiply integers
  - H2: Multiply string node X int
  - H2: Multiply int X string node
  - H2: Multiply string X int node
  - H2: Multiply int node X string
  - H2: Merge objects together, returning merged result only
  - H2: Merge objects together, returning parent object
  - H2: Merge keeps style of LHS
  - H2: Merge arrays
  - H2: Merge, only existing fields
  - H2: Merge, only new fields
  - H2: Merge, appending arrays
  - H2: Merge, only existing fields, appending arrays
  - H2: Merge, deeply merging arrays
  - H2: Merge to prefix an element
  - H2: Merge with simple aliases
  - H2: Merge copies anchor names
  - H2: Merge with merge anchors
  - H2: Custom types: that are really numbers
  - H2: Custom types: that are really maps
  - H2: Custom types: clobber tags
  - H2: Merging a null with a map
  - H2: Merging a map with null
  - H2: Merging a null with an array
  - H2: Merging an array with null

## operators/omit.md

- Route: /operators/omit
- Headings:
  - H1: Omit
  - H2: Omit keys from map
  - H2: Omit indices from array

## operators/parent.md

- Route: /operators/parent
- Headings:
  - H1: Parent
  - H2: Simple example
  - H2: Parent of nested matches
  - H2: Get parent attribute
  - H2: Get parents
  - H2: Get the top (root) parent
  - H2: Root
  - H2: N-th parent
  - H2: N-th parent - another level
  - H2: N-th negative
  - H2: No parent

## operators/path.md

- Route: /operators/path
- Headings:
  - H1: Path
  - H2: Map path
  - H2: Get map key
  - H2: Array path
  - H2: Get array index
  - H2: Print path and value
  - H2: Set path
  - H2: Set on empty document
  - H2: Set path to prune deep paths
  - H2: Set array path
  - H2: Set array path empty
  - H2: Delete path
  - H2: Delete array path
  - H2: Delete - wrong parameter

## operators/pick.md

- Route: /operators/pick
- Headings:
  - H1: Pick
  - H2: Pick keys from map
  - H2: Pick keys from map, included all the keys
  - H2: Pick indices from array

## operators/recursive-descent-glob.md

- Route: /operators/recursive-descent-glob
- Headings:
  - H1: Recursive Descent (Glob)
  - H2: match values form ..
  - H2: match values and map keys form ...
  - H2: Recurse map (values only)
  - H2: Recursively find nodes with keys
  - H2: Recursively find nodes with values
  - H2: Recurse map (values and keys)
  - H2: Aliases are not traversed
  - H2: Merge docs are not traversed

## operators/reduce.md

- Route: /operators/reduce
- Headings:
  - H1: Reduce
  - H2: treease vs jq syntax
  - H2: Sum numbers
  - H2: Convert an array to an object

## operators/relational.md

- Route: /operators/relational
- Headings:
  - H1: Relational Operators
  - H2: Related Operators
  - H2: Relational comparison of numbers (&gt;)
  - H2: Relational comparison of equal numbers (&gt;=)
  - H2: Relational comparison of strings
  - H2: Relational comparison of date times
  - H2: Both sides are null: &gt; is false
  - H2: Both sides are null: &gt;= is true

## operators/reverse.md

- Route: /operators/reverse
- Headings:
  - H1: Reverse
  - H2: Reverse
  - H2: Sort descending by string field

## operators/select.md

- Route: /operators/select
- Headings:
  - H1: Select
  - H2: Related Operators
  - H2: Select elements from array using wildcard prefix
  - H2: Select elements from array using wildcard suffix
  - H2: Select elements from array using wildcard prefix and suffix
  - H2: Select elements from array with regular expression
  - H2: Select items from a map
  - H2: Use select and withentries to filter map keys
  - H2: Select multiple items in a map and update

## operators/shuffle.md

- Route: /operators/shuffle
- Headings:
  - H1: Shuffle
  - H2: Shuffle array
  - H2: Shuffle array in place

## operators/sort-keys.md

- Route: /operators/sort-keys
- Headings:
  - H1: Sort Keys
  - H2: Sort keys of map
  - H2: Sort keys recursively

## operators/sort.md

- Route: /operators/sort
- Headings:
  - H1: Sort
  - H2: Sort by string field
  - H2: Sort by multiple fields
  - H2: Sort descending by string field
  - H2: Sort array in place
  - H2: Sort array of objects by key
  - H2: Sort a map
  - H2: Sort a map by keys
  - H2: Sort is stable
  - H2: Sort by numeric field
  - H2: Sort, nulls come first

## operators/string-operators.md

- Route: /operators/string-operators
- Headings:
  - H1: String Operators
  - H2: RegEx
  - H3: match(regEx)
  - H3: capture(regEx)
  - H2: test(regEx)
  - H2: sub(regEx, replacement)
  - H2: Interpolation
  - H2: Interpolation - not a string
  - H2: To up (upper) case
  - H2: To down (lower) case
  - H2: Join strings
  - H2: Trim strings
  - H2: Match string
  - H2: Match string, case insensitive
  - H2: Capture full match into a map
  - H2: Match without global flag
  - H2: Match with global flag
  - H2: Test using regex
  - H2: Substitute / Replace string
  - H2: Substitute / Replace string with regex
  - H2: Custom types: that are really strings
  - H2: Split strings
  - H2: Split strings one match
  - H2: To string

## operators/subtract.md

- Route: /operators/subtract
- Headings:
  - H1: Subtract
  - H2: Array subtraction
  - H2: Array subtraction with nested array
  - H2: Array subtraction with nested object
  - H2: Number subtraction - float
  - H2: Number subtraction - int
  - H2: Decrement numbers
  - H2: Date subtraction
  - H2: Custom types: that are really numbers

## operators/tag.md

- Route: /operators/tag
- Headings:
  - H1: Tag
  - H2: Get tag
  - H2: type is an alias for tag
  - H2: Set custom tag
  - H2: Find numbers and convert them to strings

## operators/to_number.md

- Route: /operators/to_number
- Headings:
  - H1: To Number
  - H2: Converts strings to numbers
  - H2: Doesn't change numbers
  - H2: Cannot convert null

## operators/traverse-read.md

- Route: /operators/traverse-read
- Headings:
  - H1: Traverse (Read)
  - H2: Simple map navigation
  - H2: Splat
  - H2: Optional Splat
  - H2: Special characters
  - H2: Nested special characters
  - H2: Keys with spaces
  - H2: Dynamic keys
  - H2: Children don't exist
  - H2: Optional identifier
  - H2: Wildcard matching
  - H2: Aliases
  - H2: Traversing aliases with splat
  - H2: Traversing aliases explicitly
  - H2: Traversing arrays by index
  - H2: Traversing nested arrays by index
  - H2: Maps with numeric keys
  - H2: Maps with non existing numeric keys
  - H2: Traversing merge anchors
  - H2: Traversing merge anchors with local override
  - H2: Select multiple indices
  - H2: LEGACY: Traversing merge anchors with override
  - H2: LEGACY: Traversing merge anchor lists
  - H2: LEGACY: Splatting merge anchors
  - H2: LEGACY: Splatting merge anchor lists
  - H2: FIXED: Traversing merge anchors with override
  - H2: FIXED: Traversing merge anchor lists
  - H2: FIXED: Splatting merge anchors
  - H2: FIXED: Splatting merge anchor lists

## operators/union.md

- Route: /operators/union
- Headings:
  - H1: Union
  - H2: Combine scalars
  - H2: Combine selected paths

## operators/unique.md

- Route: /operators/unique
- Headings:
  - H1: Unique
  - H2: Unique array of scalars (string/numbers)
  - H2: Unique nulls
  - H2: Unique all nulls
  - H2: Unique array objects
  - H2: Unique array of objects by a field
  - H2: Unique array of arrays

## operators/variable-operators.md

- Route: /operators/variable-operators
- Headings:
  - H1: Variable Operators
  - H2: Single value variable
  - H2: Multi value variable
  - H2: Using variables as a lookup
  - H2: Using variables to swap values

## operators/with.md

- Route: /operators/with
- Headings:
  - H1: With
  - H2: Update and style
  - H2: Update multiple deeply nested properties
  - H2: Update array elements relatively

## references/index.md

- Route: /references
- Headings:
  - H1: References
  - H2: Pages
  - H2: Related Layers

## references/supported-syntax-and-operators.md

- Route: /references/supported-syntax-and-operators
- Headings:
  - H1: 支持的语法、算子
  - H2: 目标
  - H2: 先说明边界
  - H2: 当前代码入口
  - H2: 表达式语法概览
  - H3: 注释与空白
  - H3: 标识符与字面量
  - H3: 分组与集合
  - H3: 路径遍历
  - H3: 变量
  - H2: 算子能力概览
  - H3: 组合与控制
  - H3: 赋值
  - H3: 算术与比较
  - H3: 集合与遍历
  - H3: 字符串与转换
  - H3: 元数据与路径
  - H3: 编解码
  - H2: 手写参考与生成快照的分工
  - H2: 维护规则

## references/yaml-common-subset.md

- Route: /references/yaml-common-subset
- Headings:
  - H1: YAML 常见子集
  - H2: 目标
  - H2: 常见 YAML 子集
  - H2: 不属于常见子集
  - H2: 当前失败集分类

## start/index.md

- Route: /start
- Headings:
  - H1: Start
  - H2: What Readers Should Learn Here
  - H2: Read Here First
  - H2: Then Continue To

## start/user-stories.md

- Route: /start/user-stories
- Headings:
  - H1: User Stories
  - H2: 说明
  - H2: 产品概览
  - H2: 目标用户
  - H2: 核心用户旅程
  - H2: 用户故事
  - H3: US-01 导入文件并自动识别格式
  - H3: US-02 在混杂文本中定位并查看 JSON 块
  - H3: US-03 在 editor 中整理文本
  - H3: US-04 在 Graph 和 Tree Path 中定位字段
  - H3: US-05 在 editor 中 hover 值节点查看预览
  - H3: US-06 在 Graph 中 click 打开局部工作区
  - H3: US-07 在 Graph 中逐层打开子图工作区
  - H3: US-08 在 Graph 和 editor 之间同步修改结果
  - H3: US-09 导出为目标格式
  - H3: US-10 比较两份内容
  - H3: US-11 通过 URL preset 打开可复现入口
  - H3: US-12 调整个人使用偏好
  - H3: US-13 处理大文件时看到进度
  - H3: US-14 在命令行中处理结构化输入
  - H2: 使用边界
  - H2: 维护规则

## testing/index.md

- Route: /testing
- Headings:
  - H1: Testing
  - H2: Core Principles
  - H2: Layer Selection
  - H2: Verification Commands
  - H2: What Requires Real-Chain Proof
  - H2: Related Domains

## web/bidirectional-edit-pipeline.md

- Route: /web/bidirectional-edit-pipeline
- Headings:
  - H1: Bidirectional Edit Pipeline
  - H2: 核心实体
  - H3: Editor Model
  - H3: Graph Interaction
  - H3: Graph Edit Planner
  - H3: Commit Transaction
  - H3: DocumentSnapshot
  - H2: 核心实体关系
  - H2: 双向编辑约束
  - H3: Editor → Graph
  - H3: Graph → Editor
  - H3: fallback
  - H2: 数据流
  - H3: 1. Editor → Graph
  - H3: 2. Graph → Editor
  - H3: 3. 子图工作区 graph pane
  - H3: 4. 子图工作区 content pane
  - H2: 子图工作区入口约束
  - H2: 检查清单

## web/editor-data-flow.md

- Route: /web/editor-data-flow
- Headings:
  - H1: Editor Data Flow
  - H2: 核心实体
  - H3: Editor Model
  - H3: Commit Transaction
  - H3: Document Runtime
  - H3: DocumentSnapshot
  - H3: Workspace Store
  - H3: Workspace Mirror Text
  - H3: Active Document Context
  - H3: View Runtime
  - H3: View Runtime Operation Lifecycle
  - H2: 核心实体关系
  - H2: 核心约束
  - H3: 文本 authority
  - H3: 提交 authority
  - H3: 语义 authority
  - H3: 绑定 authority
  - H3: View Runtime operation lifecycle
  - H2: 业务场景数据流
  - H3: 1. 用户直接编辑主文档
  - H3: 2. 程序化整文替换
  - H3: 3. 主文档语义读取
  - H3: 4. parse failed / clear graph
  - H3: 5. blank / whitespace clear
  - H2: 主文档数据流检查清单

## web/graph-stream-benchmark.md

- Route: /web/graph-stream-benchmark
- Headings:
  - H1: Web Graph Stream Benchmark
  - H2: 目的
  - H2: 冻结口径
  - H2: 运行命令
  - H2: 输出文件
  - H2: 关键指标
  - H2: 推荐规则
  - H2: 回归阈值
  - H2: 当前冻结推荐（v2）
  - H2: 当前运行时 chunk size 策略
  - H2: 风险标记
  - H2: 仍保留的边界

## web/index.md

- Route: /web
- Headings:
  - H1: Web
  - H2: Domain Rules
  - H2: What This Domain Covers
  - H2: Read By Topic
  - H2: Relation To Other Domains

## web/layout-pipeline.md

- Route: /web/layout-pipeline
- Headings:
  - H1: 布局约束
  - H2: 输入与输出
  - H3: 输入
  - H3: 输出
  - H2: 核心实体
  - H3: Topology
  - H3: Graph Node
  - H3: Graph Edge
  - H3: Table Presentation
  - H3: Layout Result
  - H2: 核心实体关系
  - H2: 一、节点判定
  - H3: Tree 结构到图结构的映射
  - H3: 独立节点规则
  - H3: sequence 分化规则
  - H3: 节点判定的一致性要求
  - H2: 二、节点表现
  - H3: Scalar
  - H3: Object
  - H3: 空容器退化
  - H3: Headerless Table
  - H3: Header Table
  - H3: Header Table 的 fallback value 列规则
  - H3: virtual table
  - H2: 三、几何规则
  - H3: X 方向规则
  - H3: Y 方向规则
  - H3: edge 规则
  - H2: 四、一致性约束
  - H3: full build / streaming 一致性
  - H3: changed-region 一致性
  - H3: 几何一致性
  - H2: 五、明确错误
  - H2: 检查清单

## web/stream-pipeline.md

- Route: /web/stream-pipeline
- Headings:
  - H1: 流式处理
  - H2: 核心实体
  - H3: Stream Input
  - H3: Document Job Session
  - H3: Stream Decoder / Builder
  - H3: Streaming Graph Projector
  - H3: Final Snapshot
  - H2: 核心实体关系
  - H2: 数据流
  - H3: same-language JSON 真流式
  - H3: 非 JSON 假流式
  - H3: ApplyEdits
  - H2: 时序位置
  - H3: enable nest parse
  - H3: enable auto format
  - H2: 实现约束
  - H3: 真流式约束
  - H3: 假流式约束
  - H3: clear / parse failed 约束
  - H3: nested JSON 约束
  - H3: auto format 约束
  - H3: root scalar replace 约束
  - H2: 检查清单

## web/subgraph-workspace.md

- Route: /web/subgraph-workspace
- Headings:
  - H1: 子图工作区
  - H2: 核心实体
  - H3: Workspace Chain
  - H3: Workspace Pane
  - H3: Graph Pane
  - H3: Content Pane
  - H3: Workspace Projection
  - H3: Local Draft
  - H3: Pending Commit
  - H2: 核心实体关系
  - H2: 子图工作区约束
  - H3: 子域定位
  - H3: pane 约束
  - H3: graph / content 分流约束
  - H3: projection 约束
  - H3: 编辑约束
  - H3: 生命周期约束
  - H2: 数据流
  - H3: 1. 主图点击打开工作区
  - H3: 2. 在 graph pane 中继续下钻
  - H3: 3. 在 content pane 中编辑
  - H3: 4. 外部主文档刷新影响工作区
  - H3: 5. 同一路径连续提交
  - H2: 工作区专有规则
  - H3: path 规则
  - H3: cache 规则
  - H2: Module Architecture
  - H3: 交互一致性规则
  - H2: 检查清单

