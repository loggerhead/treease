# Sort Keys

The Sort Keys operator sorts maps by their keys (based on their string value). This operator does not do anything to arrays or scalars (so you can easily recursively apply it to all maps).

Sort is particularly useful for diffing two different yaml documents:

```bash
treease -i -P 'sort_keys(..)' file1.yml
treease -i -P 'sort_keys(..)' file2.yml
diff file1.yml file2.yml
```

Note that `treease` does not yet consider anchors when sorting by keys - this may result in invalid yaml documents if you are using merge anchors.

For more advanced sorting, you can use the [sort_by](sort.md) function on a map, and give it a custom function like `sort_by(key | downcase)`.

## Sort keys of map

Given a sample.yml file of:

```yaml
c: frog
a: blah
b: bing
```

then

```bash
treease 'sort_keys(.)' sample.yml
```

will output

```yaml
a: blah
b: bing
c: frog
```

## Sort keys recursively

Note the array elements are left unsorted, but maps inside arrays are sorted

Given a sample.yml file of:

```yaml
bParent:
  c: dog
  array:
    - 3
    - 1
    - 2
aParent:
  z: donkey
  x:
    - c: yum
      b: delish
    - b: ew
      a: apple
```

then

```bash
treease 'sort_keys(..)' sample.yml
```

will output

```yaml
aParent:
  x:
    - b: delish
      c: yum
    - a: apple
      b: ew
  z: donkey
bParent:
  array:
    - 3
    - 1
    - 2
  c: dog
```
