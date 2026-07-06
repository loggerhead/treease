# 支持的语法、算子

## 目标
- 作为 Core 表达式能力的手写参考说明
- 说明"表达式语法可识别范围"和"已注册算子能力"的关系
- 给阅读代码、补文档和做能力对比时提供稳定入口

## 先说明边界
- 本文档覆盖“当前手写总览想让读者先知道的能力分组”。
- 本文档列出的能力不等于 `docs/operators/` 下都已有独立正文页。
- 是否存在独立正文页，以 `docs/operators/README.md` 的目录说明和实际文件为准。
- 是否被当前构建真正注册，以 `../generated/core-registry-capabilities.md` 和实际 registry 为准。

## 当前代码入口
- 词法：`packages/core/src/parser/lexer_participle.rs`
- Token 后处理：`packages/core/src/parser/lexer.rs`
- 表达式解析 / 中缀转后缀：`packages/core/src/parser/parser.rs`
- 表达式树构建：`packages/core/src/core/expression_builder.rs`
- 核心操作定义：`packages/core/src/registry/operation.rs`
- 算子注册：`packages/core/src/operators/registry.rs`
- 算子注册表：`packages/core/src/operators/registry_tables_ops.rs`
- 编解码注册表：`packages/core/src/operators/registry_tables_formats.rs`

## 表达式语法概览

### 注释与空白
- 支持空格、Tab、换行、回车
- 支持 `#` 行注释

### 标识符与字面量
- 标识符以字母或下划线开头，后续允许数字、下划线和 `-`
- 支持 `true`、`false`、`null`
- 支持整数、浮点与双引号字符串
- 字符串支持基本转义与插值相关语法入口

### 分组与集合
- `(` `)`：分组
- `[ ... ]`：数组构造与收集
- `{ ... }`：对象构造与收集
- `:`：键值对构造

### 路径遍历
- `.`：self
- `.name` / `.name?`：按 key 遍历
- `.[]` / `.[]?`：数组或集合遍历
- `..` / `...`：递归下降

### 变量
- `$foo`：变量引用
- `as`：变量绑定

## 算子能力概览

### 组合与控制
- `|`
- `;`
- `,`
- `reduce`
- `with`
- `empty`

### 赋值
- `=`
- `=c`
- `|=`
- `+=` / `-=` / `*=`

### 算术与比较
- `+` `-` `*` `/` `%`
- `//`
- `==` `!=`
- `<` `<=` `>` `>=`

### 集合与遍历
- `select`
- `map` / `map_values`
- `filter`
- `keys` / `key`
- `has`
- `contains`
- `pick`
- `omit`
- `flatten`
- `group_by`
- `unique` / `unique_by`
- `sort` / `sort_by`
- `sort_keys`
- `reverse`
- `shuffle`
- `first`
- `delpaths`

### 字符串与转换
- `sub`
- `match`
- `capture`
- `test`
- `join`
- `split`
- `trim`
- `to_string`
- `to_number`
- `upcase` / `downcase`

### 元数据与路径
- `path`
- `setpath`
- `parent`
- `parents`
- `tag` / `type`
- `kind`

### 编解码
- registry-backed 编解码格式：`yaml`、`json`、`csv`、`base64`、`toml`、`python`、`javascript`
- shorthand：`@yaml` / `@json` / `@csv` / `@base64`
- shorthand decode：`@yamld` / `@jsond` / `@csvd` / `@base64d`
- function 形式：`to_yaml` / `to_json` / `to_csv`
- function decode：`from_yaml` / `from_json` / `from_csv`
- 说明：`toml` / `python` / `javascript` 已在 codec registry 中注册，但当前词法 shorthand 与 `to_...` / `from_...` 列表仍未覆盖它们；`@uri` / `@urid` 仅有词法入口，当前不在 registry-backed 编解码清单内。

## 手写参考与生成快照的分工
- 本文档负责解释能力分组、入口文件和阅读路径
- 自动生成的注册能力快照见 `../generated/core-registry-capabilities.md`
- 如需判断某个算子是否被当前构建启用，优先以生成快照和实际注册表为准

## 维护规则
- 更新语法或算子实现时，同步核对本文档是否仍能反映当前主链路
- 需要精确核验支持清单时，结合 `packages/core/src/tools/export_registry_doc.rs` 生成的 `docs/generated/core-registry-capabilities.md` 一起检查
