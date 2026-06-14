# Traverse (Read)

This is the simplest (and perhaps most used) operator. It is used to navigate deeply into yaml structures.


Note: This version does not support YAML merge key semantics (`<<:`). `<<` is treated as a normal key, and merge-related flags/examples are obsolete (some older examples below may still mention `!!merge`).


## Simple map navigation
Given a sample.yml file of:
```yaml
a:
  b: apple
```
then
```bash
treease '.a' sample.yml
```
will output
```yaml
b: apple
```

## Splat
Often used to pipe children into other operators

Given a sample.yml file of:
```yaml
- b: apple
- c: banana
```
then
```bash
treease '.[]' sample.yml
```
will output
```yaml
b: apple
c: banana
```

## Optional Splat
Just like splat, but won't error if you run it against scalars

Given a sample.yml file of:
```yaml
cat
```
then
```bash
treease '.[]' sample.yml
```
will output
```yaml
```

## Special characters
Use quotes with square brackets around path elements with special characters

Given a sample.yml file of:
```yaml
"{}": frog
```
then
```bash
treease '.["{}"]' sample.yml
```
will output
```yaml
frog
```

## Nested special characters
Given a sample.yml file of:
```yaml
a:
  "key.withdots":
    "another.key": apple
```
then
```bash
treease '.a["key.withdots"]["another.key"]' sample.yml
```
will output
```yaml
apple
```

## Keys with spaces
Use quotes with square brackets around path elements with special characters

Given a sample.yml file of:
```yaml
"red rabbit": frog
```
then
```bash
treease '.["red rabbit"]' sample.yml
```
will output
```yaml
frog
```

## Dynamic keys
Expressions within [] can be used to dynamically lookup / calculate keys

Given a sample.yml file of:
```yaml
b: apple
apple: crispy yum
banana: soft yum
```
then
```bash
treease '.[.b]' sample.yml
```
will output
```yaml
crispy yum
```

## Children don't exist
Nodes are added dynamically while traversing

Given a sample.yml file of:
```yaml
c: banana
```
then
```bash
treease '.a.b' sample.yml
```
will output
```yaml
null
```

## Optional identifier
Like jq, does not output an error when the yaml is not an array or object as expected

Given a sample.yml file of:
```yaml
- 1
- 2
- 3
```
then
```bash
treease '.a?' sample.yml
```
will output
```yaml
```

## Wildcard matching
Given a sample.yml file of:
```yaml
a:
  cat: apple
  mad: things
```
then
```bash
treease '.a."*a*"' sample.yml
```
will output
```yaml
apple
things
```

## Aliases
Given a sample.yml file of:
```yaml
a: &cat
  c: frog
b: *cat
```
then
```bash
treease '.b' sample.yml
```
will output
```yaml
*cat
```

## Traversing aliases with splat
Given a sample.yml file of:
```yaml
a: &cat
  c: frog
b: *cat
```
then
```bash
treease '.b[]' sample.yml
```
will output
```yaml
frog
```

## Traversing aliases explicitly
Given a sample.yml file of:
```yaml
a: &cat
  c: frog
b: *cat
```
then
```bash
treease '.b.c' sample.yml
```
will output
```yaml
frog
```

## Traversing arrays by index
Given a sample.yml file of:
```yaml
- 1
- 2
- 3
```
then
```bash
treease '.[0]' sample.yml
```
will output
```yaml
1
```

## Traversing nested arrays by index
Given a sample.yml file of:
```yaml
[[], [cat]]
```
then
```bash
treease '.[1][0]' sample.yml
```
will output
```yaml
cat
```

## Maps with numeric keys
Given a sample.yml file of:
```yaml
2: cat
```
then
```bash
treease '.[2]' sample.yml
```
will output
```yaml
cat
```

## Maps with non existing numeric keys
Given a sample.yml file of:
```yaml
a: b
```
then
```bash
treease '.[0]' sample.yml
```
will output
```yaml
null
```

## Traversing merge anchors
Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobar.a' sample.yml
```
will output
```yaml
foo_a
```

## Traversing merge anchors with local override
Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobar.thing' sample.yml
```
will output
```yaml
foobar_thing
```

## Select multiple indices
Given a sample.yml file of:
```yaml
a:
  - a
  - b
  - c
```
then
```bash
treease '.a[0, 2]' sample.yml
```
will output
```yaml
a
c
```

## LEGACY: Traversing merge anchors with override
This is legacy behaviour, see --yaml-fix-merge-anchor-to-spec

Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobar.c' sample.yml
```
will output
```yaml
foo_c
```

## LEGACY: Traversing merge anchor lists
Note that the later merge anchors override previous, but this is legacy behaviour, see --yaml-fix-merge-anchor-to-spec

Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobarList.thing' sample.yml
```
will output
```yaml
bar_thing
```

## LEGACY: Splatting merge anchors
With legacy override behaviour, see --yaml-fix-merge-anchor-to-spec

Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobar[]' sample.yml
```
will output
```yaml
foo_c
foo_a
foobar_thing
```

## LEGACY: Splatting merge anchor lists
With legacy override behaviour, see --yaml-fix-merge-anchor-to-spec

Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobarList[]' sample.yml
```
will output
```yaml
bar_b
foo_a
bar_thing
foobarList_c
```

## FIXED: Traversing merge anchors with override
Set `--yaml-fix-merge-anchor-to-spec=true` to get this correct merge behaviour.

Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobar.c' sample.yml
```
will output
```yaml
foobar_c
```

## FIXED: Traversing merge anchor lists
Set `--yaml-fix-merge-anchor-to-spec=true` to get this correct merge behaviour. Note that the keys earlier in the merge anchors sequence override later ones

Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobarList.thing' sample.yml
```
will output
```yaml
foo_thing
```

## FIXED: Splatting merge anchors
Set `--yaml-fix-merge-anchor-to-spec=true` to get this correct merge behaviour. Note that the keys earlier in the merge anchors sequence override later ones

Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobar[]' sample.yml
```
will output
```yaml
foo_a
foobar_thing
foobar_c
```

## FIXED: Splatting merge anchor lists
Set `--yaml-fix-merge-anchor-to-spec=true` to get this correct merge behaviour. Note that the keys earlier in the merge anchors sequence override later ones

Given a sample.yml file of:
```yaml
foo: &foo
  a: foo_a
  thing: foo_thing
  c: foo_c
bar: &bar
  b: bar_b
  thing: bar_thing
  c: bar_c
foobarList:
  b: foobarList_b
  !!merge <<:
    - *foo
    - *bar
  c: foobarList_c
foobar:
  c: foobar_c
  !!merge <<: *foo
  thing: foobar_thing
```
then
```bash
treease '.foobarList[]' sample.yml
```
will output
```yaml
foobarList_b
foo_thing
foobarList_c
foo_a
```
