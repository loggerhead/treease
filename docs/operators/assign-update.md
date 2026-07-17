# Assign (Update)

This operator is used to update node values. It can be used in either the:

### plain form: `=`

Which will set the LHS node values equal to the RHS node values. The RHS expression is run against the matching nodes in the pipeline.

### relative form: `|=`

This will do a similar thing to the plain form, but the RHS expression is run with _each LHS node as context_. This is useful for updating values based on old values, e.g. increment.

### Flags

- `c` clobber custom tags

## Create yaml file

Running

```bash
treease --null-input '.a.b = "cat" | .x = "frog"'
```

will output

```yaml
a:
  b: cat
x: frog
```

## Update node to be the child value

Given a sample.yml file of:

```yaml
a:
  b:
    g: foof
```

then

```bash
treease '.a |= .b' sample.yml
```

will output

```yaml
a:
  g: foof
```

## Double elements in an array

Given a sample.yml file of:

```yaml
- 1
- 2
- 3
```

then

```bash
treease '.[] |= . * 2' sample.yml
```

will output

```yaml
- 2
- 4
- 6
```

## Update node to be the sibling value

Given a sample.yml file of:

```yaml
a:
  b: child
b: sibling
```

then

```bash
treease '.a = .b' sample.yml
```

will output

```yaml
a: sibling
b: sibling
```

## Updated multiple paths

Given a sample.yml file of:

```yaml
a: fieldA
b: fieldB
c: fieldC
```

then

```bash
treease '(.a, .c) = "potato"' sample.yml
```

will output

```yaml
a: potato
b: fieldB
c: potato
```

## Update string value

Given a sample.yml file of:

```yaml
a:
  b: apple
```

then

```bash
treease '.a.b = "frog"' sample.yml
```

will output

```yaml
a:
  b: frog
```

## Update string value via |=

Note there is no difference between `=` and `|=` when the RHS is a scalar

Given a sample.yml file of:

```yaml
a:
  b: apple
```

then

```bash
treease '.a.b |= "frog"' sample.yml
```

will output

```yaml
a:
  b: frog
```

## Update deeply selected results

Note that the LHS is wrapped in brackets! This is to ensure we don't first filter out the yaml and then update the snippet.

Given a sample.yml file of:

```yaml
a:
  b: apple
  c: cactus
```

then

```bash
treease '(.a[] | select(. == "apple")) = "frog"' sample.yml
```

will output

```yaml
a:
  b: frog
  c: cactus
```

## Update array values

Given a sample.yml file of:

```yaml
- candy
- apple
- sandy
```

then

```bash
treease '(.[] | select(. == "*andy")) = "bogs"' sample.yml
```

will output

```yaml
- bogs
- apple
- bogs
```

## Update empty object

Given a sample.yml file of:

```yaml
{}
```

then

```bash
treease '.a.b |= "bogs"' sample.yml
```

will output

```yaml
a:
  b: bogs
```

## Update node value that has an anchor

Anchor will remain

Given a sample.yml file of:

```yaml
a: &cool cat
```

then

```bash
treease '.a = "dog"' sample.yml
```

will output

```yaml
a: &cool dog
```

## Update empty object and array

Given a sample.yml file of:

```yaml
{}
```

then

```bash
treease '.a.b.[0] |= "bogs"' sample.yml
```

will output

```yaml
a:
  b:
    - bogs
```

## Custom types are maintained by default

Given a sample.yml file of:

```yaml
a: !cat meow
b: !dog woof
```

then

```bash
treease '.a = .b' sample.yml
```

will output

```yaml
a: !cat woof
b: !dog woof
```

## Custom types: clobber

Use the `c` option to clobber custom tags

Given a sample.yml file of:

```yaml
a: !cat meow
b: !dog woof
```

then

```bash
treease '.a =c .b' sample.yml
```

will output

```yaml
a: !dog woof
b: !dog woof
```
