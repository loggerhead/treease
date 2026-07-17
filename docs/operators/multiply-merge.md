# Multiply (Merge)

Like the multiple operator in jq, depending on the operands, this multiply operator will do different things. Currently numbers, arrays and objects are supported.

## Objects and arrays - merging

Objects are merged _deeply_ matching on matching keys. By default, array values override and are not deeply merged.

You can use the add operator `+`, to shallow merge objects, see more info [here](add.md).

Note that when merging objects, this operator returns the merged object (not the parent). This will be clearer in the examples below.

### Merge Flags

You can control how objects are merged by using one or more of the following flags. Multiple flags can be used together, e.g. `.a *+? .b`. See examples below

- `+` append arrays
- `d` deeply merge arrays
- `?` only merge _existing_ fields
- `n` only merge _new_ fields
- `c` clobber custom tags

To perform a shallow merge only, use the add operator `+`, see more info [here](add.md).

# Merging complex arrays together by a key field

By default - `treease` merge is naive. It merges maps when they match the key name, and arrays are merged either by appending them together, or merging the entries by their position in the array.

For more complex array merging (e.g. merging items that match on a certain key) please see the example [here](#merging-complex-arrays-together-by-a-key-field)

## Multiply integers

Given a sample.yml file of:

```yaml
a: 3
b: 4
```

then

```bash
treease '.a *= .b' sample.yml
```

will output

```yaml
a: 12
b: 4
```

## Multiply string node X int

Given a sample.yml file of:

```yaml
b: banana
```

then

```bash
treease '.b * 4' sample.yml
```

will output

```yaml
bananabananabananabanana
```

## Multiply int X string node

Given a sample.yml file of:

```yaml
b: banana
```

then

```bash
treease '4 * .b' sample.yml
```

will output

```yaml
bananabananabananabanana
```

## Multiply string X int node

Given a sample.yml file of:

```yaml
n: 4
```

then

```bash
treease '"banana" * .n' sample.yml
```

will output

```yaml
bananabananabananabanana
```

## Multiply int node X string

Given a sample.yml file of:

```yaml
n: 4
```

then

```bash
treease '.n * "banana"' sample.yml
```

will output

```yaml
bananabananabananabanana
```

## Merge objects together, returning merged result only

Given a sample.yml file of:

```yaml
a:
  field: me
  fieldA: cat
b:
  field:
    g: wizz
  fieldB: dog
```

then

```bash
treease '.a * .b' sample.yml
```

will output

```yaml
field:
  g: wizz
fieldA: cat
fieldB: dog
```

## Merge objects together, returning parent object

Given a sample.yml file of:

```yaml
a:
  field: me
  fieldA: cat
b:
  field:
    g: wizz
  fieldB: dog
```

then

```bash
treease '. * {"a":.b}' sample.yml
```

will output

```yaml
a:
  field:
    g: wizz
  fieldA: cat
  fieldB: dog
b:
  field:
    g: wizz
  fieldB: dog
```

## Merge keeps style of LHS

Given a sample.yml file of:

```yaml
a: { things: great }
b:
  also: "me"
```

then

```bash
treease '. * {"a":.b}' sample.yml
```

will output

```yaml
a: { things: great, also: "me" }
b:
  also: "me"
```

## Merge arrays

Given a sample.yml file of:

```yaml
a:
  - 1
  - 2
  - 3
b:
  - 3
  - 4
  - 5
```

then

```bash
treease '. * {"a":.b}' sample.yml
```

will output

```yaml
a:
  - 3
  - 4
  - 5
b:
  - 3
  - 4
  - 5
```

## Merge, only existing fields

Given a sample.yml file of:

```yaml
a:
  thing: one
  cat: frog
b:
  missing: two
  thing: two
```

then

```bash
treease '.a *? .b' sample.yml
```

will output

```yaml
thing: two
cat: frog
```

## Merge, only new fields

Given a sample.yml file of:

```yaml
a:
  thing: one
  cat: frog
b:
  missing: two
  thing: two
```

then

```bash
treease '.a *n .b' sample.yml
```

will output

```yaml
thing: one
cat: frog
missing: two
```

## Merge, appending arrays

Given a sample.yml file of:

```yaml
a:
  array:
    - 1
    - 2
    - animal: dog
  value: coconut
b:
  array:
    - 3
    - 4
    - animal: cat
  value: banana
```

then

```bash
treease '.a *+ .b' sample.yml
```

will output

```yaml
array:
  - 1
  - 2
  - animal: dog
  - 3
  - 4
  - animal: cat
value: banana
```

## Merge, only existing fields, appending arrays

Given a sample.yml file of:

```yaml
a:
  thing:
    - 1
    - 2
b:
  thing:
    - 3
    - 4
  another:
    - 1
```

then

```bash
treease '.a *?+ .b' sample.yml
```

will output

```yaml
thing:
  - 1
  - 2
  - 3
  - 4
```

## Merge, deeply merging arrays

Merging arrays deeply means arrays are merged like objects, with indices as their key. In this case, we merge the first item in the array and do nothing with the second.

Given a sample.yml file of:

```yaml
a:
  - name: fred
    age: 12
  - name: bob
    age: 32
b:
  - name: fred
    age: 34
```

then

```bash
treease '.a *d .b' sample.yml
```

will output

```yaml
- name: fred
  age: 34
- name: bob
  age: 32
```

## Merge to prefix an element

Given a sample.yml file of:

```yaml
a: cat
b: dog
```

then

```bash
treease '. * {"a": {"c": .a}}' sample.yml
```

will output

```yaml
a:
  c: cat
b: dog
```

## Merge with simple aliases

Given a sample.yml file of:

```yaml
a: &cat
  c: frog
b:
  f: *cat
c:
  g: thongs
```

then

```bash
treease '.c * .b' sample.yml
```

will output

```yaml
g: thongs
f: *cat
```

## Merge copies anchor names

Given a sample.yml file of:

```yaml
a:
  c: &cat frog
b:
  f: *cat
c:
  g: thongs
```

then

```bash
treease '.c * .a' sample.yml
```

will output

```yaml
g: thongs
c: &cat frog
```

## Merge with merge anchors

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
treease '.foobar * .foobarList' sample.yml
```

will output

```yaml
c: foobarList_c
!!merge <<:
  - *foo
  - *bar
thing: foobar_thing
b: foobarList_b
```

## Custom types: that are really numbers

When custom tags are encountered, treease will try to decode the underlying type.

Given a sample.yml file of:

```yaml
a: !horse 2
b: !goat 3
```

then

```bash
treease '.a = .a * .b' sample.yml
```

will output

```yaml
a: !horse 6
b: !goat 3
```

## Custom types: that are really maps

Custom tags will be maintained.

Given a sample.yml file of:

```yaml
a: !horse
  cat: meow
b: !goat
  dog: woof
```

then

```bash
treease '.a = .a * .b' sample.yml
```

will output

```yaml
a: !horse
  cat: meow
  dog: woof
b: !goat
  dog: woof
```

## Custom types: clobber tags

Use the `c` option to clobber custom tags. Note that the second tag is now used.

Given a sample.yml file of:

```yaml
a: !horse
  cat: meow
b: !goat
  dog: woof
```

then

```bash
treease '.a *=c .b' sample.yml
```

will output

```yaml
a: !goat
  cat: meow
  dog: woof
b: !goat
  dog: woof
```

## Merging a null with a map

Running

```bash
treease --null-input 'null * {"some": "thing"}'
```

will output

```yaml
some: thing
```

## Merging a map with null

Running

```bash
treease --null-input '{"some": "thing"} * null'
```

will output

```yaml
some: thing
```

## Merging a null with an array

Running

```bash
treease --null-input 'null * ["some"]'
```

will output

```yaml
- some
```

## Merging an array with null

Running

```bash
treease --null-input '["some"] * null'
```

will output

```yaml
- some
```
