# Variable Operators

Like the `jq` equivalents, variables are sometimes required for the more complex expressions (or swapping values between fields).

## Single value variable

Given a sample.yml file of:

```yaml
a: cat
```

then

```bash
treease '.a as $foo | $foo' sample.yml
```

will output

```yaml
cat
```

## Multi value variable

Given a sample.yml file of:

```yaml
- cat
- dog
```

then

```bash
treease '.[] as $foo | $foo' sample.yml
```

will output

```yaml
cat
dog
```

## Using variables as a lookup

Example taken from [jq](https://stedolan.github.io/jq/manual/#Variable/SymbolicBindingOperator:...as$identifier|...)

Given a sample.yml file of:

```yaml
"posts":
  - "title": First post
    "author": anon
  - "title": A well-written article
    "author": person1
"realnames":
  "anon": Anonymous Coward
  "person1": Person McPherson
```

then

```bash
treease '.realnames as $names | .posts[] | {"title":.title, "author": $names[.author]}' sample.yml
```

will output

```yaml
title: First post
author: Anonymous Coward
title: A well-written article
author: Person McPherson
```

## Using variables to swap values

Given a sample.yml file of:

```yaml
a: a_value
b: b_value
```

then

```bash
treease '.a as $x  | .b as $y | .b = $x | .a = $y' sample.yml
```

will output

```yaml
a: b_value
b: a_value
```
