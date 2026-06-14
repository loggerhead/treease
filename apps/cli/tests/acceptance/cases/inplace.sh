#!/usr/bin/env bash

test_inplace() {
  local sample="$TMP_DIR/sample.json"
  printf '{"foo":1}
' >"$sample"

  run_cli '' '-i' '.foo' "$sample"
  assert_eq 0 "$LAST_EXIT_CODE" 'inplace write should exit 0'
  assert_eq '' "$LAST_STDOUT" 'inplace write should not print stdout'
  assert_eq '' "$LAST_STDERR" 'inplace write should not print stderr'
  assert_file_eq "$sample" '1
' 'inplace write should update file contents'
}
