---
summary: "Python Dict format support, examples, and boundaries."
read_when:
  - Understanding or changing Python Dict encoding, decoding, import, or export behavior
  - Checking whether the format documentation matches the actual supported scope
---

# Python Dict

Parse Python dict or list literals into Treease's structured tree model.

This format is intended for Python-like data literals such as dictionaries, lists,
strings, booleans, `None`, and nested combinations of these values. The current
build also supports emitting Python-style output for compatible structures.

## Parse: dict literal

Given a `sample.py` file of:

```python
{
  "name": "Treease",
  "enabled": True,
  "count": 3,
}
```

then

```bash
treease -oy '.' sample.py
```

will output

```yaml
name: Treease
enabled: true
count: 3
```

## Parse: nested objects and arrays

Given a `sample.py` file of:

```python
{
  "service": {
    "hosts": ["a", "b"],
    "retries": 2,
  }
}
```

then

```bash
treease -oy '.' sample.py
```

will output

```yaml
service:
  hosts:
    - a
    - b
  retries: 2
```

## Roundtrip: preserve Python-style data

Given a `sample.py` file of:

```python
{
  "name": "Treease",
  "enabled": True,
  "items": [1, 2, 3],
  "value": None,
}
```

then

```bash
treease '.' sample.py
```

will output

```python
{"name": "Treease", "enabled": True, "items": [1, 2, 3], "value": None}
```

## Convert to another format

Given a `sample.py` file of:

```python
{
  "name": "Treease",
  "enabled": True,
}
```

then

```bash
treease -o=yaml '.' sample.py
```

will output

```yaml
name: Treease
enabled: true
```

## Notes

- This page documents Python literal support, not arbitrary Python source code.
- Actual edge-case behavior still depends on the current decoder and encoder
  implementations in `packages/core/src/formats/decoder_python.rs` and
  `packages/core/src/formats/encoder_python.rs`.
