#!/usr/bin/env bash

test_eval() {
  run_cli '{"foo":1}
' '.foo'
  assert_eq 0 "$LAST_EXIT_CODE" 'default eval should exit 0'
  assert_text_eq '1
' "$LAST_STDOUT_HEX" 'default eval should print selected value'
  assert_eq '' "$LAST_STDERR" 'default eval should not print stderr'

  run_cli '{"foo":1}
' '-e' '.missing'
  assert_eq 1 "$LAST_EXIT_CODE" 'exit-status should return 1 for missing result'
  assert_text_eq 'null
' "$LAST_STDOUT_HEX" 'missing result should render as null'

  run_cli 'foo: 1
bar: 2
' '.foo'
  assert_eq 0 "$LAST_EXIT_CODE" 'stdin yaml should be guessed without explicit input format'
  assert_text_eq '1
' "$LAST_STDOUT_HEX" 'stdin yaml guess should print selected value'

  local suffixless_yaml="$TMP_DIR/sample"
  cat >"$suffixless_yaml" <<'EOF'
foo: 1
bar: 2
EOF
  run_cli '' '.bar' "$suffixless_yaml"
  assert_eq 0 "$LAST_EXIT_CODE" 'suffixless yaml file should be guessed from content'
  assert_text_eq '2
' "$LAST_STDOUT_HEX" 'suffixless yaml file guess should print selected value'
}
