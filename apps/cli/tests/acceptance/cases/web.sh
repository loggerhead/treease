#!/usr/bin/env bash

TREEASE_WEB_PIDS=()
TREEASE_WEB_LAST_PID=''
TREEASE_WEB_ASSET_SERVER_PID=''
TREEASE_WEB_ASSET_BASE_URL=''
TREEASE_WEB_CACHE_DIR=''
TREEASE_WEB_ASSET_VERSION=''
TREEASE_WEB_ASSET_RUNTIME_VERSION='1762550945000'
TREEASE_WEB_ASSET_PORT_FILE=''

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
  local server_log="$TMP_DIR/web-assets-server.log"
  local server_port_file="$TMP_DIR/web-assets-server.port"
  TREEASE_WEB_CACHE_DIR="$TMP_DIR/web-cache"
  TREEASE_WEB_ASSET_PORT_FILE="$server_port_file"
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
    { "path": "_app/app.js" },
    { "path": "landing/hero.webp" }
  ]
}
EOF

  python3 - "$asset_root" "$server_log" "$server_port_file" <<'PY' &
import contextlib
import http.server
import pathlib
import socketserver
import sys

asset_root = pathlib.Path(sys.argv[1])
log_path = pathlib.Path(sys.argv[2])
port_path = pathlib.Path(sys.argv[3])

class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(asset_root), **kwargs)

with log_path.open("w", encoding="utf-8") as log_file:
    with contextlib.redirect_stderr(log_file), contextlib.redirect_stdout(log_file):
        class Server(socketserver.TCPServer):
            allow_reuse_address = True

        with Server(("127.0.0.1", 0), Handler) as httpd:
            port_path.write_text(f"{httpd.server_address[1]}\n", encoding="utf-8")
            httpd.serve_forever()
PY
  TREEASE_WEB_ASSET_SERVER_PID=$!
  for _ in {1..100}; do
    if [[ -s "$server_port_file" ]]; then
      local server_port
      server_port="$(cat "$server_port_file")"
      TREEASE_WEB_ASSET_BASE_URL="http://127.0.0.1:${server_port}"
      return 0
    fi
    if ! kill -0 "$TREEASE_WEB_ASSET_SERVER_PID" 2>/dev/null; then
      printf 'web asset server log:\n%s\n' "$(cat "$server_log" 2>/dev/null)" >&2
      fail "web asset server exited before reporting its port"
    fi
    sleep 0.1
  done

  printf 'web asset server log:\n%s\n' "$(cat "$server_log" 2>/dev/null)" >&2
  fail "timed out waiting for web asset server port"
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

meta_url_for_graph_url() {
  python3 - "$1" <<'PY'
import sys
print(sys.argv[1].replace("/cli/graph?", "/cli/meta?", 1))
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
from urllib.parse import urljoin

meta_file, graph_url = sys.argv[1:]
payload = json.loads(pathlib.Path(meta_file).read_text())
print(urljoin(graph_url, payload["source_url"]))
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

  fetch_url "$graph_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'web graph URL should serve embedded index'
  assert_contains "$(cat "$body_file")" '<!doctype html>' 'web graph URL should serve embedded web shell'

  fetch_url "$meta_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'web metadata should return 200 with matching token'
  assert_meta_payload "$body_file" "$input" '.foo' 'json' "$(source_path_for_meta_url "$meta_url")" 'web metadata payload should match file input'
  source_url="$(source_url_from_meta_file "$body_file" "$graph_url")"
  fetch_url "$source_url" "$body_file" "$status_file"
  assert_eq 200 "$(cat "$status_file")" 'web source should return 200 with matching token'
  assert_file_text "$body_file" $'1\n' 'web source should match file input'
  assert_contains "$(cat "$stderr_file")" $'\rDownloading web assets (' 'web should refresh download progress in place'
  assert_contains "$(cat "$stderr_file")" '_app/app.js' 'web should report downloaded asset path'
  assert_not_contains "$(cat "$stderr_file")" 'landing/' 'web should skip landing-only assets during CLI download'

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
  assert_not_contains "$LAST_STDERR" 'Downloading web assets' 'web missing file should fail before asset download'
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

  test_web_missing_file_error
}
