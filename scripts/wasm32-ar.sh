#!/usr/bin/env bash
# Cross-platform archiver for wasm32 objects built by cc-rs.
set -eu

if command -v llvm-ar >/dev/null 2>&1; then
    exec llvm-ar "$@"
fi

if [ -x /opt/homebrew/opt/llvm/bin/llvm-ar ]; then
    exec /opt/homebrew/opt/llvm/bin/llvm-ar "$@"
fi

exec ar "$@"
