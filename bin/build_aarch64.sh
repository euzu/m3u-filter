#!/usr/bin/env bash
echo "building binary for aarch64"
env RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target aarch64-unknown-linux-musl
