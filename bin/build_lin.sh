#!/usr/bin/env bash
export RUSTFLAGS="--remap-path-prefix $HOME=~"
if [ "$(uname)" != "Linux" ]; then
  cargo build --release --target x86_64-unknown-linux-musl # --target x86_64-unknown-linux-gnu
else
  cargo build --release
fi