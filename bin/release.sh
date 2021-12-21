#!/usr/bin/env bash

if [[ -z "$1" ]]; then
    echo "No argument supplied. First argument should be version number like 1.0.0"
    exit 1
fi

VERSION=$1
LIN_DIR=m3u-filter_${VERSION}_linux_x86_64
WIN_DIR=m3u-filter_${VERSION}_windows_x86_64
DARWIN_DIR=m3u-filter_${VERSION}_darwin_x86_64

LIN_ARC=${LIN_DIR}.tgz
WIN_ARC=${WIN_DIR}.zip
DARWIN_ARC=${DARWIN_DIR}.tgz

./bin/build_lin.sh && \
#./bin/build_darwin.sh && \
./bin/build_win.sh && \
./bin/build_fe.sh && \
cd target && \
rm -rf $LIN_DIR $DARWIN_DIR $WIN_DIR $LIN_ARC $DARWIN_ARC $WIN_ARC release_${VERSION} && \
mkdir $LIN_DIR && \
mkdir $WIN_DIR && \
#mkdir $DARWIN_DIR
if [ "$(uname)" != "Linux" ]; then
    cp x86_64-unknown-linux-gnu/release/m3u-filter $LIN_DIR
else
    cp release/m3u-filter $LIN_DIR
fi
if [ $? -ne 0 ]; then
  exit $?
fi
#if [ "$(uname)" != "Darwin" ]; then
#    cp x86_64-apple-darwin/release/m3u-filter $DARWIN_DIR
#else
#    cp release/m3u-filter $DARWIN_DIR
#fi
#if [ $? -ne 0 ]; then
#  exit $?
#fi
cp x86_64-pc-windows-gnu/release/m3u-filter.exe $WIN_DIR && \
cp ../config.yml $LIN_DIR && \
cp ../config.yml $WIN_DIR && \
#cp ../config.yml $DARWIN_DIR && \
cp -rf ../frontend/build $LIN_DIR/web && \
cp -rf ../frontend/build $WIN_DIR/web && \
#cp -rf ../frontend/build $DARWIN_DIR/web && \
tar cvzf $LIN_ARC $LIN_DIR && \
#tar cvzf $DARWIN_ARC $DARWIN_DIR && \
zip -r $WIN_ARC $WIN_DIR && \
shasum -a 256 $LIN_ARC > checksum.txt && \
#shasum -a 256 $DARWIN_ARC > checksum.txt && \
shasum -a 256 $WIN_ARC >> checksum.txt && \
mkdir release_${VERSION} && \
mv $LIN_ARC release_${VERSION} && \
#mv $DARWIN_ARC release_${VERSION} && \
mv $WIN_ARC  release_${VERSION} && \
mv checksum.txt release_${VERSION} && \
rm -rf $LIN_DIR $DARWIN_DIR $WIN_DIR && \
echo "done!"


