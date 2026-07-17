# Relational Operators

Relational operators (`>`, `>=`, `<`, `<=`) can be used for comparing scalar values of the same type.

The following types are currently supported:

- numbers
- strings
- datetimes

## Related Operators

- equals / not equals (`==`, `!=`) operators [here](equals.md)
- boolean operators (`and`, `or`, `any` etc) [here](boolean-operators.md)
- select operator [here](select.md)

## Relational comparison of numbers (>)

Given a sample.yml file of:

```yaml
a: 5
b: 4
```

then

```bash
treease '.a > .b' sample.yml
```

will output

```yaml
true
```

## Relational comparison of equal numbers (>=)

Given a sample.yml file of:

```yaml
a: 5
b: 5
```

then

```bash
treease '.a >= .b' sample.yml
```

will output

```yaml
true
```

## Relational comparison of strings

Compares strings by their bytecode.

Given a sample.yml file of:

```yaml
a: zoo
b: apple
```

then

```bash
treease '.a > .b' sample.yml
```

will output

```yaml
true
```

## Relational comparison of date times

You can compare date times. Assumes RFC3339 date time format.

Given a sample.yml file of:

```yaml
a: 2021-01-01T03:10:00Z
b: 2020-01-01T03:10:00Z
```

then

```bash
treease '.a > .b' sample.yml
```

will output

```yaml
true
```

## Both sides are null: > is false

Running

```bash
treease --null-input '.a > .b'
```

will output

```yaml
false
```

## Both sides are null: >= is true

Running

```bash
treease --null-input '.a >= .b'
```

will output

```yaml
true
```
