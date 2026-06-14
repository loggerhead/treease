# Subtract

You can use subtract to subtract numbers as well as remove elements from an array.

## Array subtraction
Running
```bash
treease --null-input '[1,2] - [2,3]'
```
will output
```yaml
- 1
```

## Array subtraction with nested array
Running
```bash
treease --null-input '[[1], 1, 2] - [[1], 3]'
```
will output
```yaml
- 1
- 2
```

## Array subtraction with nested object
Note that order of the keys does not matter

Given a sample.yml file of:
```yaml
- a: b
  c: d
- a: b
```
then
```bash
treease '. - [{"c": "d", "a": "b"}]' sample.yml
```
will output
```yaml
- a: b
```

## Number subtraction - float
If the lhs or rhs are floats then the expression will be calculated with floats.

Given a sample.yml file of:
```yaml
a: 3
b: 4.5
```
then
```bash
treease '.a = .a - .b' sample.yml
```
will output
```yaml
a: -1.5
b: 4.5
```

## Number subtraction - int
If both the lhs and rhs are ints then the expression will be calculated with ints.

Given a sample.yml file of:
```yaml
a: 3
b: 4
```
then
```bash
treease '.a = .a - .b' sample.yml
```
will output
```yaml
a: -1
b: 4
```

## Decrement numbers
Given a sample.yml file of:
```yaml
a: 3
b: 5
```
then
```bash
treease '.[] -= 1' sample.yml
```
will output
```yaml
a: 2
b: 4
```

## Date subtraction
You can subtract durations from dates. Assumes RFC3339 date time format.

Given a sample.yml file of:
```yaml
a: 2021-01-01T03:10:00Z
```
then
```bash
treease '.a -= "3h10m"' sample.yml
```
will output
```yaml
a: 2021-01-01T00:00:00Z
```

## Custom types: that are really numbers
When custom tags are encountered, treease will try to decode the underlying type.

Given a sample.yml file of:
```yaml
a: !horse 2
b: !goat 1
```
then
```bash
treease '.a -= .b' sample.yml
```
will output
```yaml
a: !horse 1
b: !goat 1
```

