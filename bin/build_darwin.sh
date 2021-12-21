#!/usr/bin/env bash
if [ "$(uname)" != "Darwin" ]; then
  cargo build --release --target x86_64-apple-darwin
else
  cargo build --release
fi