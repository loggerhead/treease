#!/usr/bin/env bash

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local message="$3"

  if [[ "$actual" != "$expected" ]]; then
    printf 'FAIL: %s\n' "$message" >&2
    printf 'expected:\n%s\n' "$expected" >&2
    printf 'actual:\n%s\n' "$actual" >&2
    exit 1
  fi
}

assert_text_eq() {
  local expected="$1"
  local actual_hex="$2"
  local message="$3"
  local actual_text

  if ! python3 - "$expected" "$actual_hex" <<'PY'
import sys
expected = sys.argv[1].encode()
actual = bytes.fromhex(sys.argv[2])
raise SystemExit(0 if actual == expected else 1)
PY
  then
    actual_text="$(python3 - "$actual_hex" <<'PY'
import sys
print(repr(bytes.fromhex(sys.argv[1]).decode()))
PY
)"
    printf 'FAIL: %s\n' "$message" >&2
    printf 'expected text:\n%s\n' "$(python3 - "$expected" <<'PY'
import sys
print(repr(sys.argv[1]))
PY
)" >&2
    printf 'actual text:\n%s\n' "$actual_text" >&2
    exit 1
  fi
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local message="$3"

  if [[ "$haystack" != *"$needle"* ]]; then
    printf 'FAIL: %s\n' "$message" >&2
    printf 'expected to contain: %s\n' "$needle" >&2
    printf 'actual:\n%s\n' "$haystack" >&2
    exit 1
  fi
}

assert_not_contains() {
  local haystack="$1"
  local needle="$2"
  local message="$3"

  if [[ "$haystack" == *"$needle"* ]]; then
    printf 'FAIL: %s\n' "$message" >&2
    printf 'expected not to contain: %s\n' "$needle" >&2
    printf 'actual:\n%s\n' "$haystack" >&2
    exit 1
  fi
}

assert_file_eq() {
  local path="$1"
  local expected="$2"
  local message="$3"
  local actual_hex

  actual_hex="$(python3 -c 'import pathlib,sys; sys.stdout.write(pathlib.Path(sys.argv[1]).read_bytes().hex())' "$path")"
  assert_text_eq "$expected" "$actual_hex" "$message"
}

run_cli() {
  local stdin_text="$1"
  shift

  local stdout_file="$TMP_DIR/stdout.txt"
  local stderr_file="$TMP_DIR/stderr.txt"

  set +e
  if [[ -n "$stdin_text" ]]; then
    printf '%s' "$stdin_text" | "$TREEASE_BIN" "$@" >"$stdout_file" 2>"$stderr_file"
    LAST_EXIT_CODE=$?
  else
    "$TREEASE_BIN" "$@" >"$stdout_file" 2>"$stderr_file"
    LAST_EXIT_CODE=$?
  fi
  set -e

  LAST_STDOUT="$(python3 -c 'import pathlib,sys; sys.stdout.write(pathlib.Path(sys.argv[1]).read_text())' "$stdout_file")"
  LAST_STDERR="$(python3 -c 'import pathlib,sys; sys.stdout.write(pathlib.Path(sys.argv[1]).read_text())' "$stderr_file")"
  LAST_STDOUT_HEX="$(python3 -c 'import pathlib,sys; sys.stdout.write(pathlib.Path(sys.argv[1]).read_bytes().hex())' "$stdout_file")"
  LAST_STDERR_HEX="$(python3 -c 'import pathlib,sys; sys.stdout.write(pathlib.Path(sys.argv[1]).read_bytes().hex())' "$stderr_file")"
}

begin_case() {
  local name="$1"
  TMP_DIR="$(mktemp -d "${TMP_ROOT}/${name}.XXXXXX")" || fail "failed to create temp dir for ${name}"
}

end_case() {
  rm -rf "$TMP_DIR"
}
