#!/usr/bin/env bash
# Wrapper for zig cc targeting wasm32-unknown-unknown.
# Used by cc-rs via CC_wasm32_unknown_unknown in .cargo/config.toml
#
# zig's clang does not accept 'unknown' in the OS field of --target=
# (e.g. --target=wasm32-unknown-unknown fails with UnknownOperatingSystem).
# Rewrite it to -target wasm32-freestanding, which zig understands.
set -eu

args=()
for arg in "$@"; do
    if [ "$arg" = "--target=wasm32-unknown-unknown" ]; then
        args+=(-target wasm32-freestanding)
    else
        args+=("$arg")
    fi
done

exec zig cc "${args[@]}"
