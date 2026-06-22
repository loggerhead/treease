#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/treease-crate-packaging.XXXXXX")"

cleanup() {
  rm -rf "$TMP_ROOT"
}

trap cleanup EXIT

package_crate() {
  local crate_dir="$1"
  CARGO_TARGET_DIR="$TMP_ROOT/target" \
    cargo package \
    --locked \
    --allow-dirty \
    --no-verify \
    --manifest-path "$crate_dir/Cargo.toml" \
    --target-dir "$TMP_ROOT/target" \
    >/dev/null
}

package_crate "$REPO_ROOT/packages/core"
CLI_PACKAGE_LIST="$TMP_ROOT/treease-cli-package-list.txt"
cargo package \
  --allow-dirty \
  --list \
  --manifest-path "$REPO_ROOT/apps/cli/Cargo.toml" \
  >"$CLI_PACKAGE_LIST"

grep -q '^build.rs$' "$CLI_PACKAGE_LIST" || {
  echo "CLI package list is missing build.rs" >&2
  exit 1
}

grep -q '^src/web_assets.rs$' "$CLI_PACKAGE_LIST" || {
  echo "CLI package list is missing src/web_assets.rs" >&2
  exit 1
}

grep -q '^src/bin/export_cli_metadata.rs$' "$CLI_PACKAGE_LIST" || {
  echo "CLI package list is missing src/bin/export_cli_metadata.rs" >&2
  exit 1
}
