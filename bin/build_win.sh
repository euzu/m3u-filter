#!/usr/bin/env bash
export RUSTFLAGS="--remap-path-prefix $HOME=~"
cargo build --release --target x86_64-pc-windows-gnu
