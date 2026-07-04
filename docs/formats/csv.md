---
summary: "CSV 格式的支持范围、示例与边界说明。"
read_when:
  - 需要理解或修改 CSV 编解码、导入或导出行为
  - 需要核对格式文档与实际支持范围是否一致
---
# CSV

Encode, decode, and roundtrip CSV files.

## Encode

CSV encoding supports arrays of homogeneous flat objects:

```yaml
- name: Bobo
  type: dog
- name: Fifi
  type: cat
```

It also supports arrays of arrays of scalars:

```yaml
- [Bobo, dog]
- [Fifi, cat]
```

Use `-o=csv`, `to_csv`, or `@csv` depending on the entry point.

## Decode

CSV decoding assumes the first row is the header row, and all rows beneath are entries.
Rows are decoded into an array of objects using header cells as keys.

```csv
name,type
Bobo,dog
Fifi,cat
```

Use `-p=csv`, `from_csv`, or `@csvd` depending on the entry point.

## Web import and export

In the Web editor, CSV is an import/export exchange format, not an editor language.
Importing a `.csv` file decodes it with the Core CSV decoder, converts the result into the active editor language, and then rebuilds the document through the normal full-edit import session.
The editor language remains an editor-supported language such as JSON; it is not set to `csv`.

When the active editor language is TOML, CSV rows are wrapped with stable container names before encoding:

```toml
[[rows]]
name = "Bobo"
type = "dog"
```

Exporting CSV converts the current editor document into CSV through the Core CSV encoder.

The document must be representable as homogeneous flat objects or scalar rows.
