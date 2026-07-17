# Reduce

Reduce is a powerful way to process a collection of data into a new form.

```
<exp> as $<name> reduce (<init>; <block>)
```

On the LHS we are configuring the collection of items that will be reduced `<exp>` as well as what each element will be called `$<name>`. Note that the array has been splatted into its individual elements.

On the RHS there is `<init>`, the starting value of the accumulator and `<block>`, the expression that will update the accumulator for each element in the collection. Note that within the block expression, `.` will evaluate to the current value of the accumulator.

## treease vs jq syntax

`treease` currently supports the infix form shown above. jq-style prefix syntax such as `reduce .[] as $item (0; . + $item)` is not currently supported.

## Sum numbers

Given a sample.yml file of:

```yaml
- 10
- 2
- 5
- 3
```

then

```bash
treease '.[] as $item reduce (0; . + $item)' sample.yml
```

will output

```yaml
20
```

## Convert an array to an object

Given a sample.yml file of:

```yaml
- name: Cathy
  has: apples
- name: Bob
  has: bananas
```

then

```bash
treease '.[] as $item reduce ({}; .[$item | .name] = ($item | .has) )' sample.yml
```

will output

```yaml
Cathy: apples
Bob: bananas
```
