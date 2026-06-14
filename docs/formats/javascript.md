# JavaScript Object

Parse JavaScript object or array literals into Treease's structured tree model.

This format is intended for JavaScript-like data literals such as objects, arrays,
strings, numbers, booleans, `null`, and nested combinations of these values. The
current build also supports emitting JavaScript-style output for compatible structures.

## Parse: object literal

Given a `sample.js` file of:

```javascript
{
  name: "Treease",
  enabled: true,
  count: 3,
}
```

then

```bash
treease -oy '.' sample.js
```

will output

```yaml
name: Treease
enabled: true
count: 3
```

## Parse: nested structures

Given a `sample.js` file of:

```javascript
{
  service: {
    hosts: ["a", "b"],
    retries: 2,
  },
}
```

then

```bash
treease -oy '.' sample.js
```

will output

```yaml
service:
  hosts:
    - a
    - b
  retries: 2
```

## Roundtrip: preserve JavaScript-style data

Given a `sample.js` file of:

```javascript
{
  name: "Treease",
  enabled: true,
  items: [1, 2, 3],
  value: null,
}
```

then

```bash
treease '.' sample.js
```

will output

```javascript
{"name":"Treease","enabled":true,"items":[1,2,3],"value":null}
```

## Convert to another format

Given a `sample.js` file of:

```javascript
{
  name: "Treease",
  enabled: true,
}
```

then

```bash
treease -o=toml '.' sample.js
```

will output

```toml
name = "Treease"
enabled = true
```

## Notes

- This page documents JavaScript literal support, not arbitrary JavaScript programs.
- Actual edge-case behavior still depends on the current decoder and encoder
  implementations in `packages/core/src/formats/decoder_javascript.rs` and
  `packages/core/src/formats/encoder_javascript.rs`.
