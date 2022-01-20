#!/usr/bin/env bash
export RUSTFLAGS="--remap-path-prefix $HOME=~"
if [ "$(uname)" != "Darwin" ]; then
  cargo build --release --target x86_64-apple-darwin
else
  cargo build --release
fi