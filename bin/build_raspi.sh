#!/usr/bin/env bash
env RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target armv7-unknown-linux-musleabihf
