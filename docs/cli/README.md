# Treease CLI

Treease CLI keeps the execution path short:

```bash
treease '.a.b' file.yaml
```

Default stdout contains data output only. Discovery and diagnostics use explicit commands.

## Tasks

| Task | Command |
| --- | --- |
| Read a value | `treease '.a.b' file.yaml` |
| Convert YAML to JSON | `treease -p yaml -o json '.' file.yaml` |
| Write a file in place | `treease -i '.a = 1' file.yaml` |
| Inspect CLI schema | `treease help --format json` |
| List operators | `treease operators list` |
| Inspect one operator | `treease operators get select --format json` |
| List formats | `treease formats list --format json` |
| Find examples | `treease examples search "filter array"` |
| Check CLI capabilities | `treease doctor --format json` |

## Machine-Readable Files

- `../generated/cli-help.json`
- `../generated/operators.json`
- `../generated/formats.json`

## Error Codes

CLI errors include stable codes in stderr text.

- `UNKNOWN_FLAG`
- `UNKNOWN_COMMAND`
- `MISSING_VALUE`
- `INVALID_FLAG_COMBINATION`
- `UNSUPPORTED_FORMAT`
- `UNSUPPORTED_OPERATOR`
- `EXECUTION_ERROR`
- `IO_ERROR`
