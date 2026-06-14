#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
CORE_DIR="$REPO_ROOT/packages/core"

usage() {
  cat <<'EOF' >&2
Usage: test.sh [all|core|wasm|fixtures]

Commands:
  all       Run all Rust tests in packages/core.
  core      Run Rust library tests in packages/core.
  wasm      Run Rust wasm_phase_b tests in packages/core.
  fixtures  Run cargo nextest --test corpus_runner in packages/core.
EOF
}

run_core_tests() {
  printf '==> packages/core: cargo nextest run --lib\n'
  (
    cd "$CORE_DIR"
    cargo nextest run --lib
  )
}

run_wasm_tests() {
  printf '==> packages/core: cargo test --test wasm_phase_b -- --test-threads=1\n'
  (
    cd "$CORE_DIR"
    cargo test --test wasm_phase_b -- --test-threads=1
  )
}

run_fixture_tests() {
  printf '==> packages/core: cargo nextest run --test corpus_runner --no-capture\n'
  (
    cd "$CORE_DIR"
    cargo nextest run --test corpus_runner --no-capture
  )
}

run_fixture_summary_test() {
  printf '==> packages/core: cargo nextest run -E '"'"'not test(~corpus_fixtures_run)'"'"'\n'
  (
    cd "$CORE_DIR"
    cargo nextest run -E 'not test(~corpus_fixtures_run)'
  )

  printf '==> packages/core: cargo nextest run --test corpus_runner -E '"'"'test(corpus_fixtures_run)'"'"' --no-capture\n'
  (
    cd "$CORE_DIR"
    TREEASE_CORPUS_WORKERS=2 cargo nextest run --test corpus_runner -E 'test(corpus_fixtures_run)' --no-capture
  )
}

if [[ $# -gt 1 ]]; then
  usage
  exit 1
fi

mode="${1:-all}"

case "$mode" in
  all)
    run_fixture_summary_test
    ;;
  core)
    run_core_tests
    ;;
  wasm)
    run_wasm_tests
    ;;
  fixtures)
    run_fixture_tests
    ;;
  *)
    usage
    exit 1
    ;;
esac
