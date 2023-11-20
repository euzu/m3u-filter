#!/usr/bin/env bash
export RUSTFLAGS="--remap-path-prefix $HOME=~"
if [ "$(uname)" != "Linux" ]; then
  cross build --release --target --target x86_64-unknown-linux-gnu
else
  cargo build --release
fi