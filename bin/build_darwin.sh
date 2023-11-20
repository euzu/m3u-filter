#!/usr/bin/env bash
env RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target x86_64-apple-darwin
