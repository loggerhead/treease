#!/usr/bin/env bash

TREEASE_WEB_PIDS=()
TREEASE_WEB_LAST_PID=''

cleanup_web_servers() {
  local pid
  for pid in "${TREEASE_WEB_PIDS[@]:-}"; do
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    fi
  done
  TREEASE_WEB_PIDS=()

}

start_web_file() {
  local stdout_file="$1"
  local stderr_file="$2"
  shift 2

  "$TREEASE_BIN" web "$@" >"$stdout_file" 2>"$stderr_file" &
  local pid=$!
  TREEASE_WEB_PIDS+=("$pid")
  TREEASE_WEB_LAST_PID="$pid"
}

start_web_stdin() {
  local stdin_text="$1"
  local stdout_file="$2"
  local stderr_file="$3"
  shift 3

  printf '%s' "$stdin_text" | \
    "$TREEASE_BIN" web "$@" >"$stdout_file" 2>"$stderr_file" &
  local pid=$!
  TREEASE_WEB_PIDS+=("$pid")
  TREEASE_WEB_LAST_PID="$pid"
}

wait_for_graph_url() {
  local stdout_file="$1"
  local stderr_file="$2"
  local pid="$3"
  local url

  for _ in {1..100}; do
    url="$(python3 - "$stdout_file" <<'PY'
import pathlib
import re
import sys

text = pathlib.Path(sys.argv[1]).read_text(errors="replace")
match = re.search(r"https://treease\.com/editor\?textUrl=[^&]+&lang=[^&]+&ui=editor%2Cviewer", text)
print(match.group(0) if match else "")
PY
)"
    if [[ -n "$url" ]]; then
      printf '%s\n' "$url"
      return 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      printf 'stdout:\n%s\n' "$(cat "$stdout_file")" >&2
      printf 'stderr:\n%s\n' "$(cat "$stderr_file")" >&2
      fail "treease web exited before printing graph URL"
    fi
    sleep 0.1
  done

  printf 'stdout:\n%s\n' "$(cat "$stdout_file")" >&2
  printf 'stderr:\n%s\n' "$(cat "$stderr_file")" >&2
  fail "timed out waiting for treease web graph URL"
}

meta_url_for_graph_url() {
  python3 - "$1" <<'PY'
from urllib.parse import parse_qs, unquote, urlparse
import sys
parsed = urlparse(sys.argv[1])
query = parse_qs(parsed.query)
source_url = unquote(query['textUrl'][0])
print(source_url.replace('/cli/source?', '/cli/meta?', 1))
PY
}

wrong_token_url_for() {
  python3 - "$1" <<'PY'
import re
import sys
print(re.sub(r"token=[^&]+", "token=wrong-token", sys.argv[1], count=1))
PY
}

source_path_for_meta_url() {
  python3 - "$1" <<'PY'
import sys
from urllib.parse import urlparse

parsed = urlparse(sys.argv[1])
print(f"/cli/source?{parsed.query}")
PY
}

fetch_url() {
  local url="$1"
  local body_file="$2"
  local status_file="$3"

  python3 - "$url" "$body_file" "$status_file" <<'PY'
import pathlib
import sys
import urllib.error
import urllib.request

url, body_file, status_file = sys.argv[1:]
try:
    with urllib.request.urlopen(url, timeout=5) as response:
        status = response.status
        body = response.read()
except urllib.error.HTTPError as error:
    status = error.code
    body = error.read()

pathlib.Path(body_file).write_bytes(body)
pathlib.Path(status_file).write_text(str(status))
PY
}

assert_meta_payload() {
  local json_file="$1"
  local expected_source_label="$2"
  local expected_expression="$3"
  local expected_language="$4"
  local expected_source_url="$5"
  local message="$6"

  python3 - "$json_file" "$expected_source_label" "$expected_expression" "$expected_language" "$expected_source_url" "$message" <<'PY'
import json
import pathlib
import sys

json_file, source_label, expression, language, source_url, message = sys.argv[1:]
payload = json.loads(pathlib.Path(json_file).read_text())
expected = {
    "source_label": source_label,
    "expression": expression,
    "language": language,
    "source_url": source_url,
}
for key, value in expected.items():
    if payload.get(key) != value:
        print(f"FAIL: {message}", file=sys.stderr)
        print(f"field {key}: expected {value!r}, got {payload.get(key)!r}", file=sys.stderr)
        raise SystemExit(1)
PY
}

assert_file_text() {
  local text_file="$1"
  local expected_text="$2"
  local message="$3"

  python3 - "$text_file" "$expected_text" "$message" <<'PY'
import pathlib
import sys

text_file, expected_text, message = sys.argv[1:]
actual = pathlib.Path(text_file).read_text()
if actual != expected_text:
    print(f"FAIL: {message}", file=sys.stderr)
    print(f"expected {expected_text!r}, got {actual!r}", file=sys.stderr)
    raise SystemExit(1)
PY
}

source_url_from_meta_file() {
  python3 - "$1" "$2" <<'PY'
import json
import pathlib
import sys
from urllib.parse import parse_qs, unquote, urljoin, urlparse

meta_file, graph_url = sys.argv[1:]
payload = json.loads(pathlib.Path(meta_file).read_text())
parsed = urlparse(graph_url)
print(unquote(parse_qs(parsed.query)["textUrl"][0]))
PY
}

test_web_file_result() {
  local input="$TMP_DIR/input.yaml"
  local stdout_file="$TMP_DIR/web-file.stdout"
  local stderr_file="$TMP_DIR/web-file.stderr"
  local body_file="$TMP_DIR/web-file.body"
  local status_file="$TMP_DIR/web-file.status"
  local pid graph_url meta_url source_url wrong_url

  printf 'foo: 1\nbar: 2\n' >"$input"

  start_web_file "$stdout_file" "$stderr_file" -o json '.foo' "$input"
  pid="$TREEASE_WEB_LAST_PID"
  graph_url="$(wait_for_graph_url "$stdout_file" "$stderr_file" "$pid")"
  meta_url="$(meta_url_for_graph_url "$graph_url")"

  fetch_url "$meta_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'web metadata should return 200 with matching token'
  assert_meta_payload "$body_file" "$input" '.foo' 'json' "$(source_path_for_meta_url "$meta_url")" 'web metadata payload should match file input'
  source_url="$(source_url_from_meta_file "$body_file" "$graph_url")"
  fetch_url "$source_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'web source should return 200 with matching token'
  assert_file_text "$body_file" $'1\n' 'web source should match file input'
  wrong_url="$(wrong_token_url_for "$meta_url")"
  fetch_url "$wrong_url" "$body_file" "$status_file"
  assert_eq 403 "$(cat "$status_file")" 'web metadata should reject wrong token'
}

test_web_stdin_result() {
  local stdout_file="$TMP_DIR/web-stdin.stdout"
  local stderr_file="$TMP_DIR/web-stdin.stderr"
  local body_file="$TMP_DIR/web-stdin.body"
  local status_file="$TMP_DIR/web-stdin.status"
  local pid graph_url meta_url source_url

  start_web_stdin $'foo: 1\n' "$stdout_file" "$stderr_file" '.' '-'
  pid="$TREEASE_WEB_LAST_PID"
  graph_url="$(wait_for_graph_url "$stdout_file" "$stderr_file" "$pid")"
  meta_url="$(meta_url_for_graph_url "$graph_url")"

  fetch_url "$meta_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'stdin web metadata should return 200 with matching token'
  assert_meta_payload "$body_file" '<stdin>' '.' 'yaml' "$(source_path_for_meta_url "$meta_url")" 'web metadata payload should match stdin input'
  source_url="$(source_url_from_meta_file "$body_file" "$graph_url")"
  fetch_url "$source_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'stdin web source should return 200 with matching token'
  assert_file_text "$body_file" $'foo: 1\n' 'web source payload should match stdin input'
}

test_web_multiple_files_error() {
  local first="$TMP_DIR/first.yaml"
  local second="$TMP_DIR/second.yaml"
  printf 'foo: 1\n' >"$first"
  printf 'foo: 2\n' >"$second"

  run_cli '' 'web' '.' "$first" "$second"
  assert_eq 1 "$LAST_EXIT_CODE" 'web with multiple files should exit 1'
  assert_contains "$LAST_STDERR" 'INVALID_WEB_INPUT_COUNT' 'web multiple files error should include stable code'
}

test_web_missing_file_error() {
  run_cli '' 'web' '.' "$TMP_DIR/missing.yaml"
  assert_eq 1 "$LAST_EXIT_CODE" 'web with missing file should exit 1'
  assert_contains "$LAST_STDERR" 'IO_ERROR' 'web missing file should include stable IO error code'
  assert_not_contains "$LAST_STDERR" 'web graph' 'web missing file should fail before starting the server'
}

test_web() {
  test_web_file_result
  cleanup_web_servers

  test_web_stdin_result
  cleanup_web_servers

  test_web_multiple_files_error

  test_web_missing_file_error
}
