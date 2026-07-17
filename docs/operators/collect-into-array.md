# Collect into Array

This creates an array using the expression between the square brackets.

## Collect empty

Running

```bash
treease --null-input '[]'
```

will output

```yaml
[]
```

## Collect single

Running

```bash
treease --null-input '["cat"]'
```

will output

```yaml
- cat
```

## Collect many

Given a sample.yml file of:

```yaml
a: cat
b: dog
```

then

```bash
treease '[.a, .b]' sample.yml
```

will output

```yaml
- cat
- dog
```
