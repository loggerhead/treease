# Keys

Use the `keys` operator to return map keys or array indices.

## Map keys

Given a sample.yml file of:

```yaml
dog: woof
cat: meow
```

then

```bash
treease 'keys' sample.yml
```

will output

```yaml
- dog
- cat
```

## Array keys

Given a sample.yml file of:

```yaml
- apple
- banana
```

then

```bash
treease 'keys' sample.yml
```

will output

```yaml
- 0
- 1
```

## Retrieve array key

Given a sample.yml file of:

```yaml
- 1
- 2
- 3
```

then

```bash
treease '.[1] | key' sample.yml
```

will output

```yaml
1
```

## Retrieve map key

Given a sample.yml file of:

```yaml
a: thing
```

then

```bash
treease '.a | key' sample.yml
```

will output

```yaml
a
```

## No key

Given a sample.yml file of:

```yaml
{}
```

then

```bash
treease 'key' sample.yml
```

will output

```yaml

```

## Update map key

Given a sample.yml file of:

```yaml
a:
  x: 3
  y: 4
```

then

```bash
treease '(.a.x | key) = "meow"' sample.yml
```

will output

```yaml
a:
  meow: 3
  y: 4
```

## Check node is a key

Given a sample.yml file of:

```yaml
a:
  b:
    - cat
  c: frog
```

then

```bash
treease '[... | { "p": path | join("."), "isKey": is_key, "tag": tag }]' sample.yml
```

will output

```yaml
- p: ""
  isKey: false
  tag: "!!map"
- p: a
  isKey: true
  tag: "!!str"
- p: a
  isKey: false
  tag: "!!map"
- p: a.b
  isKey: true
  tag: "!!str"
- p: a.b
  isKey: false
  tag: "!!seq"
- p: a.b.0
  isKey: false
  tag: "!!str"
- p: a.c
  isKey: true
  tag: "!!str"
- p: a.c
  isKey: false
  tag: "!!str"
```
