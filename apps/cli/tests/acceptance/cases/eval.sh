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
}
