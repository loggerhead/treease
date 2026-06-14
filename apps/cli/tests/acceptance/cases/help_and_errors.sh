#!/usr/bin/env bash

test_help_and_errors() {
  run_cli '' '--help'
  assert_eq 0 "$LAST_EXIT_CODE" 'root help should exit 0'
  assert_contains "$LAST_STDOUT" 'Usage:' 'root help should include usage'
  assert_contains "$LAST_STDOUT" "treease [OPTIONS] [EXPRESSION] [FILE]..." 'root help should include default invocation'
  assert_not_contains "$LAST_STDOUT" 'treease eval' 'root help should not include eval subcommand'
  assert_not_contains "$LAST_STDOUT" 'eval-all' 'root help should not include eval-all subcommand'
  assert_contains "$LAST_STDOUT" '--help' 'root help should include help flag'
  assert_eq '' "$LAST_STDERR" 'root help should not print stderr'

  run_cli '' '-i' '.foo' '-'
  assert_eq 1 "$LAST_EXIT_CODE" 'inplace with stdin should exit 1'
  assert_contains "$LAST_STDERR" "--inplace" 'inplace stdin error should mention flag'
  assert_contains "$LAST_STDERR" 'stdin' 'inplace stdin error should mention stdin'

  run_cli '' '-i' '.foo' 'a.yml' 'b.yml'
  assert_eq 1 "$LAST_EXIT_CODE" 'inplace with multiple files should exit 1'
  assert_contains "$LAST_STDERR" 'requires exactly one input file' 'inplace multiple files error should explain constraint'

  run_cli '' 'e'
  assert_eq 1 "$LAST_EXIT_CODE" 'removed legacy e command should exit 1'
  assert_contains "$LAST_STDERR" 'UNKNOWN_COMMAND' 'removed legacy e command should include stable error code'
  assert_contains "$LAST_STDERR" 'legacy eval subcommands were removed' 'removed legacy e command should include migration hint'

  run_cli '' 'eval-all'
  assert_eq 1 "$LAST_EXIT_CODE" 'removed legacy eval-all command should exit 1'
  assert_contains "$LAST_STDERR" 'unknown command: eval-all' 'removed legacy eval-all command should mention command name'

  run_cli '' 'ea'
  assert_eq 1 "$LAST_EXIT_CODE" 'removed legacy ea command should exit 1'
  assert_contains "$LAST_STDERR" 'unknown command: ea' 'removed legacy ea command should mention command name'
}
