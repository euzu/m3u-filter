#!/usr/bin/env bash
# env RUSTFLAGS="--remap-path-prefix $HOME=~" cargo build --release --target x86_64-pc-windows-gnu
env RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target x86_64-pc-windows-gnu
