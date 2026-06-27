#!/usr/bin/env bash

TREEASE_WEB_PIDS=()
TREEASE_WEB_LAST_PID=''
TREEASE_WEB_ASSET_SERVER_PID=''
TREEASE_WEB_ASSET_BASE_URL=''
TREEASE_WEB_CACHE_DIR=''
TREEASE_WEB_ASSET_VERSION=''
TREEASE_WEB_ASSET_RUNTIME_VERSION='1762550945000'

cleanup_web_servers() {
  local pid
  for pid in "${TREEASE_WEB_PIDS[@]:-}"; do
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    fi
  done
  TREEASE_WEB_PIDS=()

  if [[ -n "$TREEASE_WEB_ASSET_SERVER_PID" ]] && kill -0 "$TREEASE_WEB_ASSET_SERVER_PID" 2>/dev/null; then
    kill "$TREEASE_WEB_ASSET_SERVER_PID" 2>/dev/null || true
    wait "$TREEASE_WEB_ASSET_SERVER_PID" 2>/dev/null || true
  fi
  TREEASE_WEB_ASSET_SERVER_PID=''
}

start_web_file() {
  local stdout_file="$1"
  local stderr_file="$2"
  shift 2

  TREEASE_WEB_ASSET_BASE_URL="$TREEASE_WEB_ASSET_BASE_URL" \
    TREEASE_WEB_CACHE_DIR="$TREEASE_WEB_CACHE_DIR" \
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
    TREEASE_WEB_ASSET_BASE_URL="$TREEASE_WEB_ASSET_BASE_URL" \
    TREEASE_WEB_CACHE_DIR="$TREEASE_WEB_CACHE_DIR" \
    "$TREEASE_BIN" web "$@" >"$stdout_file" 2>"$stderr_file" &
  local pid=$!
  TREEASE_WEB_PIDS+=("$pid")
  TREEASE_WEB_LAST_PID="$pid"
}

prepare_web_assets() {
  TREEASE_WEB_ASSET_VERSION="$(python3 - <<'PY'
import pathlib
import re

cargo = pathlib.Path("Cargo.toml").read_text()
match = re.search(r'wasm_release_date\s*=\s*"([0-9]{8})"', cargo)
if not match:
    raise SystemExit("missing wasm_release_date")
print(match.group(1))
PY
)"

  local asset_root="$TMP_DIR/web-assets"
  local version_dir="$asset_root/$TREEASE_WEB_ASSET_VERSION"
  TREEASE_WEB_CACHE_DIR="$TMP_DIR/web-cache"
  mkdir -p "$version_dir/_app"

  cat >"$version_dir/index.html" <<EOF
<!doctype html>
<html data-treease-cli-asset-version="$TREEASE_WEB_ASSET_RUNTIME_VERSION">
  <head>
    <meta charset="utf-8" />
    <script src="/_app/app.js"></script>
  </head>
  <body>graph</body>
</html>
EOF
  printf 'console.log("graph")\n' >"$version_dir/_app/app.js"
  cat >"$version_dir/manifest.json" <<EOF
{
  "version": "$TREEASE_WEB_ASSET_VERSION",
  "assetVersion": "$TREEASE_WEB_ASSET_RUNTIME_VERSION",
  "files": [
    { "path": "index.html" },
    { "path": "_app/app.js" }
  ]
}
EOF

  (cd "$asset_root" && python3 -m http.server 18766 >"$TMP_DIR/web-assets-server.log" 2>&1) &
  TREEASE_WEB_ASSET_SERVER_PID=$!
  TREEASE_WEB_ASSET_BASE_URL="http://127.0.0.1:18766"
  sleep 1
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
match = re.search(r"http://127\.0\.0\.1:[0-9]+/cli/graph\?token=[A-Za-z0-9._~-]+", text)
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

result_url_for_graph_url() {
  python3 - "$1" <<'PY'
import sys
print(sys.argv[1].replace("/cli/graph?", "/cli/result?", 1))
PY
}

wrong_token_url_for() {
  python3 - "$1" <<'PY'
import re
import sys
print(re.sub(r"token=[^&]+", "token=wrong-token", sys.argv[1], count=1))
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

assert_result_payload() {
  local json_file="$1"
  local expected_source_label="$2"
  local expected_expression="$3"
  local expected_language="$4"
  local expected_text="$5"
  local message="$6"

  python3 - "$json_file" "$expected_source_label" "$expected_expression" "$expected_language" "$expected_text" "$message" <<'PY'
import json
import pathlib
import sys

json_file, source_label, expression, language, text, message = sys.argv[1:]
payload = json.loads(pathlib.Path(json_file).read_text())
expected = {
    "source_label": source_label,
    "expression": expression,
    "language": language,
    "text": text,
}
for key, value in expected.items():
    if payload.get(key) != value:
        print(f"FAIL: {message}", file=sys.stderr)
        print(f"field {key}: expected {value!r}, got {payload.get(key)!r}", file=sys.stderr)
        raise SystemExit(1)
PY
}

test_web_file_result() {
  local input="$TMP_DIR/input.yaml"
  local stdout_file="$TMP_DIR/web-file.stdout"
  local stderr_file="$TMP_DIR/web-file.stderr"
  local body_file="$TMP_DIR/web-file.body"
  local status_file="$TMP_DIR/web-file.status"
  local pid graph_url result_url wrong_url

  printf 'foo: 1\nbar: 2\n' >"$input"

  start_web_file "$stdout_file" "$stderr_file" -o json '.foo' "$input"
  pid="$TREEASE_WEB_LAST_PID"
  graph_url="$(wait_for_graph_url "$stdout_file" "$stderr_file" "$pid")"
  result_url="$(result_url_for_graph_url "$graph_url")"

  fetch_url "$graph_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'web graph URL should serve embedded index'
  assert_contains "$(cat "$body_file")" '<!doctype html>' 'web graph URL should serve embedded web shell'

  fetch_url "$result_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'web result should return 200 with matching token'
  assert_result_payload "$body_file" "$input" '.foo' 'json' $'1\n' 'web result payload should match file input'

  wrong_url="$(wrong_token_url_for "$result_url")"
  fetch_url "$wrong_url" "$body_file" "$status_file"
  assert_eq 403 "$(cat "$status_file")" 'web result should reject wrong token'
}

test_web_stdin_result() {
  local stdout_file="$TMP_DIR/web-stdin.stdout"
  local stderr_file="$TMP_DIR/web-stdin.stderr"
  local body_file="$TMP_DIR/web-stdin.body"
  local status_file="$TMP_DIR/web-stdin.status"
  local pid graph_url result_url

  start_web_stdin $'foo: 1\n' "$stdout_file" "$stderr_file" '.' '-'
  pid="$TREEASE_WEB_LAST_PID"
  graph_url="$(wait_for_graph_url "$stdout_file" "$stderr_file" "$pid")"
  result_url="$(result_url_for_graph_url "$graph_url")"

  fetch_url "$result_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'stdin web result should return 200 with matching token'
  assert_result_payload "$body_file" '<stdin>' '.' 'yaml' $'foo: 1\n' 'web result payload should match stdin input'
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

test_web() {
  prepare_web_assets

  test_web_file_result
  cleanup_web_servers

  prepare_web_assets
  test_web_stdin_result
  cleanup_web_servers

  prepare_web_assets
  test_web_multiple_files_error
}
