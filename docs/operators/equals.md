# Equals / Not Equals

This is a boolean operator that will return `true` if the LHS is equal to the RHS and `false` otherwise.

```
.a == .b
```

It is most often used with the select operator to find particular nodes:

```
select(.a == .b)
```

The not equals `!=` operator returns `false` if the LHS is equal to the RHS.

## Related Operators

- relational (`>`, `>=`, `<`, `<=`) operators [here](relational.md)
- boolean operators (`and`, `or`, `any` etc) [here](boolean-operators.md)
- select operator [here](select.md)


## Match string
Given a sample.yml file of:
```yaml
- cat
- goat
- dog
```
then
```bash
treease '.[] | (. == "*at")' sample.yml
```
will output
```yaml
true
true
false
```

## Don't match string
Given a sample.yml file of:
```yaml
- cat
- goat
- dog
```
then
```bash
treease '.[] | (. != "*at")' sample.yml
```
will output
```yaml
false
false
true
```

## Match number
Given a sample.yml file of:
```yaml
- 3
- 4
- 5
```
then
```bash
treease '.[] | (. == 4)' sample.yml
```
will output
```yaml
false
true
false
```

## Don't match number
Given a sample.yml file of:
```yaml
- 3
- 4
- 5
```
then
```bash
treease '.[] | (. != 4)' sample.yml
```
will output
```yaml
true
false
true
```

## Match nulls
Running
```bash
treease --null-input 'null == ~'
```
will output
```yaml
true
```

## Non existent key doesn't equal a value
Given a sample.yml file of:
```yaml
a: frog
```
then
```bash
treease 'select(.b != "thing")' sample.yml
```
will output
```yaml
a: frog
```

## Two non existent keys are equal
Given a sample.yml file of:
```yaml
a: frog
```
then
```bash
treease 'select(.b == .c)' sample.yml
```
will output
```yaml
a: frog
```
