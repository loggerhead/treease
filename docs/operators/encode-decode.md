# Encoder / Decoder

Encode operators take the piped object structure and encode it as a string in the desired format. Decode operators do the reverse and parse a formatted string into the corresponding structure.

Note that you can optionally pass an indent value to the encode functions (see below).

These operators are useful when a document contains stringified embedded YAML/JSON/CSV or Base64 content.

| Format | Decode (from string) | Encode (to string) |
| --- | -- | -- |
| YAML | from_yaml / @yamld | to_yaml(i) / @yaml |
| JSON | from_json / @jsond | to_json(i) / @json |
| CSV | from_csv / @csvd | to_csv / @csv |
| Base64 | @base64d | @base64 |

This page documents the registry-backed codec capabilities that are currently usable in expressions. The parser currently recognizes `@uri` / `@urid`, but the active codec registry does not register a URI encoder/decoder, so URI is intentionally omitted here.

See CSV [documentation](../formats/csv.md) for accepted formats.


Base64 assumes [rfc4648](https://rfc-editor.org/rfc/rfc4648.html) encoding. Encoding and decoding both assume that the content is a utf-8 string and not binary content.

## Encode value as json string
Given a sample.yml file of:
```yaml
a:
  cool: thing
```
then
```bash
treease '.b = (.a | to_json)' sample.yml
```
will output
```yaml
a:
  cool: thing
b: |
  {
    "cool": "thing"
  }
```

## Encode value as json string, on one line
Pass in a 0 indent to print json on a single line.

Given a sample.yml file of:
```yaml
a:
  cool: thing
```
then
```bash
treease '.b = (.a | to_json(0))' sample.yml
```
will output
```yaml
a:
  cool: thing
b: '{"cool":"thing"}'
```

## Encode value as json string, on one line shorthand
Pass in a 0 indent to print json on a single line.

Given a sample.yml file of:
```yaml
a:
  cool: thing
```
then
```bash
treease '.b = (.a | @json)' sample.yml
```
will output
```yaml
a:
  cool: thing
b: '{"cool":"thing"}'
```

## Decode a json encoded string
Keep in mind JSON is a subset of YAML. If you want idiomatic yaml, pipe through the style operator to clear out the JSON styling.

Given a sample.yml file of:
```yaml
a: '{"cool":"thing"}'
```
then
```bash
treease '.a | from_json | ... style=""' sample.yml
```
will output
```yaml
cool: thing
```

## Decode csv encoded string
Given a sample.yml file of:
```yaml
a: |-
  cats,dogs
  great,cool as well
```
then
```bash
treease '.a |= @csvd' sample.yml
```
will output
```yaml
a:
  - cats: great
    dogs: cool as well
```

## Encode value as yaml string
Indent defaults to 2

Given a sample.yml file of:
```yaml
a:
  cool:
    bob: dylan
```
then
```bash
treease '.b = (.a | to_yaml)' sample.yml
```
will output
```yaml
a:
  cool:
    bob: dylan
b: |
  cool:
    bob: dylan
```

## Encode value as yaml string, with custom indentation
You can specify the indentation level as the first parameter.

Given a sample.yml file of:
```yaml
a:
  cool:
    bob: dylan
```
then
```bash
treease '.b = (.a | to_yaml(8))' sample.yml
```
will output
```yaml
a:
  cool:
    bob: dylan
b: |
  cool:
          bob: dylan
```

## Decode a yaml encoded string
Given a sample.yml file of:
```yaml
a: 'foo: bar'
```
then
```bash
treease '.b = (.a | from_yaml)' sample.yml
```
will output
```yaml
a: 'foo: bar'
b:
  foo: bar
```

## Update a multiline encoded yaml string
Given a sample.yml file of:
```yaml
a: |
  foo: bar
  baz: dog

```
then
```bash
treease '.a |= (from_yaml | .foo = "cat" | to_yaml)' sample.yml
```
will output
```yaml
a: |
  foo: cat
  baz: dog
```

## Update a single line encoded yaml string
Given a sample.yml file of:
```yaml
a: 'foo: bar'
```
then
```bash
treease '.a |= (from_yaml | .foo = "cat" | to_yaml)' sample.yml
```
will output
```yaml
a: 'foo: cat'
```

## Encode array of scalars as csv string
Scalars are strings, numbers and booleans.

Given a sample.yml file of:
```yaml
- cat
- thing1,thing2
- true
- 3.40
```
then
```bash
treease '@csv' sample.yml
```
will output
```yaml
cat,"thing1,thing2",true,3.40
```

## Encode array of arrays as csv string
Given a sample.yml file of:
```yaml
- - cat
  - thing1,thing2
  - true
  - 3.40
- - dog
  - thing3
  - false
  - 12
```
then
```bash
treease '@csv' sample.yml
```
will output
```yaml
cat,"thing1,thing2",true,3.40
dog,thing3,false,12
```

## Encode a string to base64
Given a sample.yml file of:
```yaml
coolData: a special string
```
then
```bash
treease '.coolData | @base64' sample.yml
```
will output
```yaml
YSBzcGVjaWFsIHN0cmluZw==
```

## Encode a yaml document to base64
Pipe through @yaml first to convert to a string, then use @base64 to encode it.

Given a sample.yml file of:
```yaml
a: apple
```
then
```bash
treease '@yaml | @base64' sample.yml
```
will output
```yaml
YTogYXBwbGUK
```

## Decode a base64 encoded string
Decoded data is assumed to be a string.

Given a sample.yml file of:
```yaml
coolData: V29ya3Mgd2l0aCBVVEYtMTYg8J+Yig==
```
then
```bash
treease '.coolData | @base64d' sample.yml
```
will output
```yaml
Works with UTF-16 😊
```

## Decode a base64 encoded yaml document
Pipe through `from_yaml` to parse the decoded base64 string as a yaml document.

Given a sample.yml file of:
```yaml
coolData: YTogYXBwbGUK
```
then
```bash
treease '.coolData |= (@base64d | from_yaml)' sample.yml
```
will output
```yaml
coolData:
  a: apple
```
