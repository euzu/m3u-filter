#!/bin/bash

if [[ -z "$1" ]]; then
    echo "No argument supplied. First argument should be version number like v1.0.0"
    exit 1
fi

VERSION=$1
LIN_DIR=m3u-filter_${VERSION}_linux_x86_64
WIN_DIR=m3u-filter_${VERSION}_windows_x86_64
LIN_ARC=${LIN_DIR}.tgz
WIN_ARC=${WIN_DIR}.zip

./bin/build_lin.sh && \
./bin/build_win.sh && \
cd target && \
rm -rf $LIN_DIR $WIN_DIR $LIN_ARC $WIN_ARC release_${VERSION} && \
mkdir $LIN_DIR && \
mkdir $WIN_DIR && \
cp release/m3u-filter $LIN_DIR && \
cp x86_64-pc-windows-gnu/release/m3u-filter.exe $WIN_DIR && \
cp ../config.yml $LIN_DIR && \
cp ../config.yml $WIN_DIR && \
tar cvzf $LIN_ARC $LIN_DIR && \
zip -r $WIN_ARC $WIN_DIR && \
shasum -a 256 $LIN_ARC > checksum.txt && \
shasum -a 256 $WIN_ARC >> checksum.txt && \
mkdir release_${VERSION} && \
mv $LIN_ARC release_${VERSION} && \
mv $WIN_ARC  release_${VERSION} && \
mv checksum.txt release_${VERSION} && \
rm -rf $LIN_DIR $WIN_DIR && \
echo "done!"


