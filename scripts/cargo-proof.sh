#!/usr/bin/env bash
# Source from isolated proof runners. A renamed filter must fail, not prove zero tests.
cargo_proof() {
  local selected_tests
  selected_tests="$(cargo test "$@" --list)" || return "$?"
  if ! grep -Eq '^[^[:space:]].*: test$' <<< "$selected_tests"; then
    echo "Rust proof selected no tests; refusing an empty success" >&2
    return 1
  fi
  cargo test "$@"
}
