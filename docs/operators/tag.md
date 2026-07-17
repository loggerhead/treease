# Tag

The tag operator can be used to get or set the tag of nodes (e.g. `!!str`, `!!int`, `!!bool`).

## Get tag

Given a sample.yml file of:

```yaml
a: cat
b: 5
c: 3.2
e: true
f: []
```

then

```bash
treease '.. | tag' sample.yml
```

will output

```yaml
!!map
!!str
!!int
!!float
!!bool
!!seq
```

## type is an alias for tag

Given a sample.yml file of:

```yaml
a: cat
b: 5
c: 3.2
e: true
f: []
```

then

```bash
treease '.. | type' sample.yml
```

will output

```yaml
!!map
!!str
!!int
!!float
!!bool
!!seq
```

## Set custom tag

Given a sample.yml file of:

```yaml
a: str
```

then

```bash
treease '.a tag = "!!mikefarah"' sample.yml
```

will output

```yaml
a: !!mikefarah str
```

## Find numbers and convert them to strings

Given a sample.yml file of:

```yaml
a: cat
b: 5
c: 3.2
e: true
```

then

```bash
treease '(.. | select(tag == "!!int")) tag= "!!str"' sample.yml
```

will output

```yaml
a: cat
b: "5"
c: 3.2
e: true
```
