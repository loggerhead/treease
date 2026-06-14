# String Operators

## RegEx
This uses Golang's native regex functions under the hood - See their [docs](https://github.com/google/re2/wiki/Syntax) for the supported syntax.

Case insensitive tip: prefix the regex with `(?i)` - e.g. `test("(?i)cats")`.

### match(regEx)
This operator returns the substring match details of the given regEx. The current implementation returns `string`, `offset`, `length`, and an empty `captures` array.

### capture(regEx)
Capture returns the full matched substring in a map under key `"0"`. Named capture groups are not currently exposed.

## test(regEx)
Returns true if the string matches the RegEx, false otherwise.

## sub(regEx, replacement)
Substitutes matched substrings. The first parameter is the regEx to match substrings within the original string. The second parameter specifies what to replace those matches with. The replacement currently supports `$$` for a literal `$` and `$0` for the full matched substring.

## Interpolation
Given a sample.yml file of:
```yaml
value: things
another: stuff
```
then
```bash
treease '.message = "I like \(.value) and \(.another)"' sample.yml
```
will output
```yaml
value: things
another: stuff
message: I like things and stuff
```

## Interpolation - not a string
Given a sample.yml file of:
```yaml
value:
  an: apple
```
then
```bash
treease '.message = "I like \(.value)"' sample.yml
```
will output
```yaml
value:
  an: apple
message: 'I like an: apple'
```

## To up (upper) case
Currently this only performs ASCII case conversion.

Given a sample.yml file of:
```yaml
cat
```
then
```bash
treease 'upcase' sample.yml
```
will output
```yaml
CAT
```

## To down (lower) case
Currently this only performs ASCII case conversion.

Given a sample.yml file of:
```yaml
CAT
```
then
```bash
treease 'downcase' sample.yml
```
will output
```yaml
cat
```

## Join strings
Given a sample.yml file of:
```yaml
- cat
- meow
- 1
- null
- true
```
then
```bash
treease 'join("; ")' sample.yml
```
will output
```yaml
cat; meow; 1; ; true
```

## Trim strings
Given a sample.yml file of:
```yaml
- ' cat'
- 'dog '
- ' cow cow '
- horse
```
then
```bash
treease '.[] | trim' sample.yml
```
will output
```yaml
cat
dog
cow cow
horse
```

## Match string
Given a sample.yml file of:
```yaml
foo bar foo
```
then
```bash
treease 'match("foo")' sample.yml
```
will output
```yaml
string: foo
offset: 0
length: 3
captures: []
```

## Match string, case insensitive
Given a sample.yml file of:
```yaml
foo bar FOO
```
then
```bash
treease '[match("(?i)foo"; "g")]' sample.yml
```
will output
```yaml
- string: foo
  offset: 0
  length: 3
  captures: []
- string: FOO
  offset: 8
  length: 3
  captures: []
```

## Capture full match into a map
Given a sample.yml file of:
```yaml
xyzzy-14
```
then
```bash
treease 'capture("[a-z]+-[0-9]+")' sample.yml
```
will output
```yaml
"0": xyzzy-14
```

## Match without global flag
Given a sample.yml file of:
```yaml
cat cat
```
then
```bash
treease 'match("cat")' sample.yml
```
will output
```yaml
string: cat
offset: 0
length: 3
captures: []
```

## Match with global flag
Given a sample.yml file of:
```yaml
cat cat
```
then
```bash
treease '[match("cat"; "g")]' sample.yml
```
will output
```yaml
- string: cat
  offset: 0
  length: 3
  captures: []
- string: cat
  offset: 4
  length: 3
  captures: []
```

## Test using regex
Like jq's equivalent, this works like match but only returns true/false instead of full match details

Given a sample.yml file of:
```yaml
- cat
- dog
```
then
```bash
treease '.[] | test("at")' sample.yml
```
will output
```yaml
true
false
```

## Substitute / Replace string
This uses Golang's regex, described [here](https://github.com/google/re2/wiki/Syntax).
Note the use of `|=` to run in context of the current string value.

Given a sample.yml file of:
```yaml
a: dogs are great
```
then
```bash
treease '.a |= sub("dogs", "cats")' sample.yml
```
will output
```yaml
a: cats are great
```

## Substitute / Replace string with regex
This uses Golang's regex, described [here](https://github.com/google/re2/wiki/Syntax).
Note the use of `|=` to run in context of the current string value.

Given a sample.yml file of:
```yaml
a: cat
b: heat
```
then
```bash
treease '.[] |= sub("a", "$0r")' sample.yml
```
will output
```yaml
a: cart
b: heart
```

## Custom types: that are really strings
When custom tags are encountered, treease will try to decode the underlying type.

Given a sample.yml file of:
```yaml
a: !horse cat
b: !goat heat
```
then
```bash
treease '.[] |= sub("a", "$0r")' sample.yml
```
will output
```yaml
a: !horse cart
b: !goat heart
```

## Split strings
Given a sample.yml file of:
```yaml
cat; meow; 1; ; true
```
then
```bash
treease 'split("; ")' sample.yml
```
will output
```yaml
- cat
- meow
- "1"
- ""
- "true"
```

## Split strings one match
Given a sample.yml file of:
```yaml
word
```
then
```bash
treease 'split("; ")' sample.yml
```
will output
```yaml
- word
```

## To string
Note that you may want to force `treease` to leave scalar values wrapped by passing in `--unwrapScalar=false` or `-r=f`

Given a sample.yml file of:
```yaml
- 1
- true
- null
- ~
- cat
- an: object
- - array
  - 2
```
then
```bash
treease '.[] |= to_string' sample.yml
```
will output
```yaml
- "1"
- "true"
- "null"
- "~"
- cat
- "an: object"
- "- array\n- 2"
```

