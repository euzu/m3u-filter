#!/usr/bin/env bash
set -e
set -o pipefail

WORKING_DIR=$(pwd)

rustup target add x86_64-unknown-linux-musl
rustup target add x86_64-pc-windows-gnu
rustup target add armv7-unknown-linux-musleabihf
rustup target add aarch64-unknown-linux-musl

if ! command -v cargo-set-version &> /dev/null
then
    echo "cargo-set-version could not be found. install it with  'cargo install cargo-edit'"
    exit
fi

cd ./frontend || (echo "cant find frontend directory" && exit)

if [ "$1" = "m" ]; then
  NEW_VERSION=$(yarn version --no-git-tag-version --major | grep "New version" | grep -Po "(\d+\.)+\d+")
elif [ "$1" = "p" ]; then
  NEW_VERSION=$(yarn version --no-git-tag-version --minor | grep "New version" | grep -Po "(\d+\.)+\d+")
else
  NEW_VERSION=$(yarn version --no-git-tag-version --patch | grep "New version" | grep -Po "(\d+\.)+\d+")
fi

cd "$WORKING_DIR"

cargo set-version "$NEW_VERSION"

VERSION=v$NEW_VERSION

echo "Building version $VERSION"

declare -A DIRS=(
    [LINUX]=m3u-filter_${VERSION}_linux_x86_64
    [WINDOWS]=m3u-filter_${VERSION}_windows_x86_64
    [RASPI]=m3u-filter_${VERSION}_armv7_raspi
    [RASPI4]=m3u-filter_${VERSION}_aarch64_raspi
)

declare -A ARCS=(
    [LINUX]=${DIRS[LINUX]}.tgz
    [WINDOWS]=${DIRS[WINDOWS]}.zip
    [RASPI]=${DIRS[RASPI]}.tgz
    [RASPI4]=${DIRS[RASPI4]}.tgz
)

declare -A BINARIES=(
    [LINUX]=x86_64-unknown-linux-musl/release/m3u-filter
    [WINDOWS]=x86_64-pc-windows-gnu/release/m3u-filter.exe
    [RASPI]=armv7-unknown-linux-musleabihf/release/m3u-filter
    [RASPI4]=aarch64-unknown-linux-musl/release/m3u-filter
)

cd "$WORKING_DIR"
# Build binaries
./bin/build_lin_static.sh
cd "$WORKING_DIR"
./bin/build_raspi.sh
cd "$WORKING_DIR"
./bin/build_win.sh
cd "$WORKING_DIR"
./bin/build_fe.sh
cd "$WORKING_DIR"

cd target

# Clean up previous builds
rm -rf "${DIRS[@]}" "${ARCS[@]}" release_"${VERSION}"

# Create directories and copy binaries and configuration files
for PLATFORM in "${!DIRS[@]}"; do
    DIR=${DIRS[$PLATFORM]}
    BIN=${BINARIES[$PLATFORM]}
    ARC=${ARCS[$PLATFORM]}

    mkdir "$DIR"
    cp "$BIN" "$DIR"
    cp ../config/*.yml "$DIR"
    cp -rf ../frontend/build "$DIR"/web

    if [[ $PLATFORM == "WINDOWS" ]]; then
        zip -r "$ARC" "$DIR"
    else
        tar cvzf "$ARC" "$DIR"
    fi

    shasum -a 256 "$ARC" >> checksum.txt
done

# Create release directory and move archives
RELEASE_DIR="release_${VERSION}"
mkdir "$RELEASE_DIR"
mv "${ARCS[@]}" checksum.txt "$RELEASE_DIR"

# Clean up build directories
rm -rf "${DIRS[@]}"

cd "${WORKING_DIR}";

# Commit and tag release
git add .
git commit -m "release ${VERSION}"
git tag -a "$VERSION" -m "$VERSION"
git push
git push --tags

echo "Done!"
