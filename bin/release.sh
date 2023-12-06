#!/usr/bin/env bash

if ! command -v cargo-set-version &> /dev/null
then
    echo "cargo-set-version could not be found. install it with  'cargo install cargo-edit'"
    exit
fi

cd ./frontend || (echo "cant find frontend directory" && exit)
NEW_VERSION=$(yarn version --no-git-tag-version --patch | grep "New version" | grep -Po "(\d+\.)+\d+")
cd ..
cargo set-version "$NEW_VERSION"

VERSION=v$NEW_VERSION

echo "building version $VERSION"

LIN_DIR=m3u-filter_${VERSION}_linux_x86_64
WIN_DIR=m3u-filter_${VERSION}_windows_x86_64
DARWIN_DIR=m3u-filter_${VERSION}_darwin_x86_64
RASPI_DIR=m3u-filter_${VERSION}_armv7_raspi

LIN_ARC=${LIN_DIR}.tgz
WIN_ARC=${WIN_DIR}.zip
DARWIN_ARC=${DARWIN_DIR}.tgz
RASPI_ARC=${RASPI_DIR}.tgz

./bin/build_lin_static.sh && \
# ./bin/build_darwin.sh && \
./bin/build_raspi.sh && \
./bin/build_win.sh && \
./bin/build_fe.sh && \
cd target && \
rm -rf "$LIN_DIR" "$RASPI_DIR" "$DARWIN_DIR" "$WIN_DIR" "$LIN_ARC" "$RASPI_ARC" "$DARWIN_ARC" "$WIN_ARC" release_"${VERSION}" && \
mkdir "$LIN_DIR" && \
mkdir "$WIN_DIR" && \
mkdir "$RASPI_DIR" && \
# mkdir "$DARWIN_DIR" && \
cp x86_64-unknown-linux-musl/release/m3u-filter "$LIN_DIR" && \
# cp x86_64-apple-darwin/release/m3u-filter "$DARWIN_DIR" && \
cp armv7-unknown-linux-musleabihf/release/m3u-filter "$RASPI_DIR" && \
cp x86_64-pc-windows-gnu/release/m3u-filter.exe "$WIN_DIR" && \
cp ../*.yml "$LIN_DIR" && \
cp ../*.yml "$WIN_DIR" && \
# cp ../*.yml "$DARWIN_DIR" && \
cp ../*.yml "$RASPI_DIR" && \
cp -rf ../frontend/build "$LIN_DIR"/web && \
cp -rf ../frontend/build "$WIN_DIR"/web && \
# cp -rf ../frontend/build "$DARWIN_DIR"/web && \
cp -rf ../frontend/build "$RASPI_DIR"/web && \
tar cvzf "$LIN_ARC" "$LIN_DIR" && \
#  tar cvzf "$DARWIN_ARC" "$DARWIN_DIR" && \
tar cvzf "$RASPI_ARC" "$RASPI_DIR" && \
zip -r "$WIN_ARC" "$WIN_DIR" && \
shasum -a 256 "$LIN_ARC" > checksum.txt && \
# shasum -a 256 "$DARWIN_ARC" >> checksum.txt && \
shasum -a 256 "$RASPI_ARC" >> checksum.txt && \
shasum -a 256 "$WIN_ARC" >> checksum.txt && \
mkdir "release_${VERSION}" && \
mv "$LIN_ARC" "release_${VERSION}" && \
# mv "$DARWIN_ARC" "release_${VERSION}" && \
mv "$RASPI_ARC" "release_${VERSION}" && \
mv "$WIN_ARC"  "release_${VERSION}" && \
mv checksum.txt "release_${VERSION}" && \
# rm -rf "$LIN_DIR" "$DARWIN_DIR" "$RASPI_DIR" "$WIN_DIR" && \
rm -rf "$LIN_DIR" "$RASPI_DIR" "$WIN_DIR" && \
git add . && \
git commit -m "release ${VERSION}" && \
git tag -a "$VERSION" -m "$VERSION" && \
git push && \
git push --tags
echo "done!"


