#!/usr/bin/env bash
if [ "$(uname)" != "Linux" ]; then
  cargo build --release --target x86_64-unknown-linux-gnu
else
  cargo build --release
fi