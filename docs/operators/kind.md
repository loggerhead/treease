# Kind

The `kind` operator identifies the type of a node as either `scalar`, `map`, or `seq`.

This can be used for filtering or transforming nodes based on their type.

Note that `null` values are treated as `scalar`.

## Get kind
Given a sample.yml file of:
```yaml
a: cat
b: 5
c: 3.2
e: true
f: []
g: {}
h: null
```
then
```bash
treease '.. | kind' sample.yml
```
will output
```yaml
map
scalar
scalar
scalar
scalar
seq
map
scalar
```

## Get kind, ignores custom tags
Unlike tag, kind is not affected by custom tags.

Given a sample.yml file of:
```yaml
a: !!thing cat
b: !!foo {}
c: !!bar []
```
then
```bash
treease '.. | kind' sample.yml
```
will output
```yaml
map
scalar
map
seq
```
