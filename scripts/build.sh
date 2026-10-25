#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if ! command -v cargo >/dev/null 2>&1 && [ -x "$HOME/.cargo/bin/cargo" ]; then
    PATH="$HOME/.cargo/bin:$PATH"
    export PATH
fi
# This machine's optional user-level Zig wrappers; normal systems use their C compiler.
if ! command -v cc >/dev/null 2>&1 && [ -x .build-tools/cc ]; then
    PATH="$PWD/.build-tools:$PATH"
    CC="$PWD/.build-tools/cc"
    AR="$PWD/.build-tools/ar"
    export PATH CC AR
fi
cargo build --release --locked -j 2 "$@"
