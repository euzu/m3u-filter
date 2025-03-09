#!/usr/bin/env bash
DEFAULT_TARGET="x86_64-unknown-linux-musl"
TARGET="${1:-$DEFAULT_TARGET}"
env RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target "$TARGET"
