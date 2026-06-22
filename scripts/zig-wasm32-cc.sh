#!/usr/bin/env bash
# Wrapper for zig cc targeting wasm32-unknown-unknown.
# Used by cc-rs via CC_wasm32_unknown_unknown in .cargo/config.toml.
#
# zig's clang does not accept `unknown` in the OS field of `--target=`.
# Rewrite it to `-target wasm32-wasi`, which provides the libc headers that
# tree-sitter grammars expect during C compilation on Zig 0.16.
set -eu

args=()
for arg in "$@"; do
    if [ "$arg" = "--target=wasm32-unknown-unknown" ]; then
        args+=(-target wasm32-wasi)
    else
        args+=("$arg")
    fi
done

exec zig cc "${args[@]}"
