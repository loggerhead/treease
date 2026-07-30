#!/usr/bin/env bash
# Wrapper for zig cc targeting wasm32-unknown-unknown.
# Used by cc-rs via CC_wasm32_unknown_unknown in .cargo/config.toml.
#
# zig's clang does not accept `unknown` in the OS field of `--target=`.
# Rewrite it to `-target wasm32-wasi`, which provides the libc headers that
# tree-sitter grammars expect during C compilation on Zig 0.16.
set -eu

args=()
has_target=0
expect_target_value=0
for arg in "$@"; do
    if [ "$expect_target_value" -eq 1 ]; then
        if [ "$arg" = "wasm32-unknown-unknown" ]; then
            args+=(wasm32-wasi)
        else
            args+=("$arg")
        fi
        expect_target_value=0
    elif [ "$arg" = "--target=wasm32-unknown-unknown" ]; then
        args+=(-target wasm32-wasi)
        has_target=1
    elif [ "$arg" = "-target" ] || [ "$arg" = "--target" ]; then
        args+=("$arg")
        has_target=1
        expect_target_value=1
    else
        args+=("$arg")
    fi
done

if [ "$has_target" -eq 0 ]; then
    args=(-target wasm32-wasi "${args[@]}")
fi

exec zig cc "${args[@]}"
