#!/usr/bin/env bash
echo "building binary for  raspi armv7"
env RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target armv7-unknown-linux-musleabihf
