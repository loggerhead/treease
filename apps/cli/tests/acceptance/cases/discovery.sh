#!/usr/bin/env bash

test_discovery() {
  run_cli '' 'help' '--format' 'json'
  assert_eq 0 "$LAST_EXIT_CODE" 'json help should exit 0'
  assert_json_field_eq "$LAST_STDOUT" 'name' 'treease' 'json help should include command name'
  assert_contains "$LAST_STDOUT" '"operators"' 'json help should include operators command'
  assert_eq '' "$LAST_STDERR" 'json help should not print stderr'

  run_cli '' 'operators' 'get' 'select' '--format' 'json'
  assert_eq 0 "$LAST_EXIT_CODE" 'operator get should exit 0'
  assert_json_field_eq "$LAST_STDOUT" 'name' 'select' 'operator get should include name'
  assert_json_field_eq "$LAST_STDOUT" 'category' 'special' 'operator get should include category'
  assert_eq '' "$LAST_STDERR" 'operator get should not print stderr'

  run_cli '' 'formats' 'get' 'yaml' '--format' 'json'
  assert_eq 0 "$LAST_EXIT_CODE" 'format get should exit 0'
  assert_json_field_eq "$LAST_STDOUT" 'name' 'yaml' 'format get should include yaml'
  assert_json_field_eq "$LAST_STDOUT" 'can_decode' 'True' 'format get should report decode support'
  assert_eq '' "$LAST_STDERR" 'format get should not print stderr'

  run_cli '' 'examples' 'get' 'filter-array' '--format' 'json'
  assert_eq 0 "$LAST_EXIT_CODE" 'example get should exit 0'
  assert_json_field_eq "$LAST_STDOUT" 'name' 'filter-array' 'example get should include name'
  assert_contains "$LAST_STDOUT" "treease '.[] | select(.enabled)' sample.yml" 'example should include command'

  run_cli '' 'doctor' '--format' 'json'
  assert_eq 0 "$LAST_EXIT_CODE" 'doctor should exit 0'
  assert_json_field_eq "$LAST_STDOUT" 'binary' 'treease' 'doctor should include binary name'

  run_cli '' '--bad-flag'
  assert_eq 1 "$LAST_EXIT_CODE" 'unknown flag should exit 1'
  assert_contains "$LAST_STDERR" 'UNKNOWN_FLAG' 'unknown flag should include stable error code'
  assert_contains "$LAST_STDERR" 'treease --help' 'unknown flag should include hint'
}
