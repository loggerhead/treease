#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../../../.." && pwd)"
CLI_DIR="$REPO_ROOT/apps/cli"

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/treease-acceptance.XXXXXX")"
trap 'rm -rf "$TMP_ROOT"' EXIT

source "$SCRIPT_DIR/helpers.sh"

cargo build --locked --manifest-path "$CLI_DIR/Cargo.toml" --bin treease >/dev/null
TREEASE_BIN="$CLI_DIR/target/debug/treease"
[[ -x "$TREEASE_BIN" ]] || fail "treease binary not found: $TREEASE_BIN"

source "$SCRIPT_DIR/cases/eval.sh"
source "$SCRIPT_DIR/cases/help_and_errors.sh"
source "$SCRIPT_DIR/cases/discovery.sh"
source "$SCRIPT_DIR/cases/inplace.sh"

run_case() {
  local name="$1"
  shift
  printf '==> %s\n' "$name"
  begin_case "$name"
  "$@"
  end_case
}

run_case "eval" test_eval
run_case "help-and-errors" test_help_and_errors
run_case "discovery" test_discovery
run_case "inplace" test_inplace

printf 'Acceptance tests passed.\n'
