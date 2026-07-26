# Divide

Divide behaves differently according to the type of the LHS:

- strings: split by the divider
- number: arithmetic division

## String split

Given a sample.yml file of:

```yaml
a: cat_meow
b: _
```

then

```bash
treease '.c = .a / .b' sample.yml
```

will output

```yaml
a: cat_meow
b: _
c:
  - cat
  - meow
```

## Number division

When both operands are integers and the division is exact, the result remains an integer. Otherwise, the result is a float.

Given a sample.yml file of:

```yaml
a: 12
b: 2
```

then

```bash
treease '.a = .a / .b' sample.yml
```

will output

```yaml
a: 6
b: 2
```

## Number division by zero

Dividing by zero results in +Inf or -Inf

Given a sample.yml file of:

```yaml
a: 1
b: -1
```

then

```bash
treease '.a = .a / 0 | .b = .b / 0' sample.yml
```

will output

```yaml
a: !!float +Inf
b: !!float -Inf
```
