# Min / Max

Computes the minimum or maximum among an incoming sequence of scalar values.

## Min

### Minimum int
Given a sample.yml file of:
```yaml
- 99
- 16
- 12
- 6
- 66
```
then
```bash
treease 'min' sample.yml
```
will output
```yaml
6
```

### Minimum string
Given a sample.yml file of:
```yaml
- foo
- bar
- baz
```
then
```bash
treease 'min' sample.yml
```
will output
```yaml
bar
```

### Minimum of empty
Given a sample.yml file of:
```yaml
[]
```
then
```bash
treease 'min' sample.yml
```
will output
```yaml
```

## Max

### Maximum int
Given a sample.yml file of:
```yaml
- 99
- 16
- 12
- 6
- 66
```
then
```bash
treease 'max' sample.yml
```
will output
```yaml
99
```

### Maximum string
Given a sample.yml file of:
```yaml
- foo
- bar
- baz
```
then
```bash
treease 'max' sample.yml
```
will output
```yaml
foo
```

### Maximum of empty
Given a sample.yml file of:
```yaml
[]
```
then
```bash
treease 'max' sample.yml
```
will output
```yaml
```