#!/usr/bin/env bash
set -e
set -o pipefail

WORKING_DIR=$(pwd)
RELEASE_DIR="$WORKING_DIR/release"
FRONTEND_DIR="${WORKING_DIR}/frontend"

if ! command -v cargo-set-version &> /dev/null
then
    echo "cargo-set-version could not be found. Install it with 'cargo install cargo-edit'"
    exit 1
fi

cd "$FRONTEND_DIR" || (echo "Can't find frontend directory" && exit 1)

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

declare -A TARGETS=(
    [LINUX]=x86_64-unknown-linux-musl
    [WINDOWS]=x86_64-pc-windows-gnu
    [ARM7]=armv7-unknown-linux-musleabihf
    [AARCH64]=aarch64-unknown-linux-musl
    # [DARWIN]=x86_64-apple-darwin
)

declare -A DIRS=(
    [LINUX]=m3u-filter_${VERSION}_linux_x86_64
    [WINDOWS]=m3u-filter_${VERSION}_windows_x86_64
    [ARM7]=m3u-filter_${VERSION}_armv7
    [AARCH64]=m3u-filter_${VERSION}_aarch64_x86_64
    [DARWIN]=m3u-filter_${VERSION}_apple-darwin_x86_64
)

# Special case mapping for binary extensions (e.g., Windows needs .exe)
declare -A BIN_EXTENSIONS=(
    [WINDOWS]=.exe
)

cd "$WORKING_DIR"
mkdir -p "$RELEASE_DIR"

# Clean previous builds
cargo clean || true

cd "$FRONTEND_DIR" && rm -rf build && yarn  && yarn build
# Check if the frontend build directory exists
if [ ! -d "$FRONTEND_DIR/build" ]; then
    echo "Error: Web directory '$FRONTEND_DIR/build' does not exist."
    exit 1
fi

cd "$WORKING_DIR"

# Build binaries
for PLATFORM in "${!TARGETS[@]}"; do
    TARGET=${TARGETS[$PLATFORM]}
    DIR=${DIRS[$PLATFORM]}
    ARC=${DIR}.tgz
    # Handle platform-specific binary file names
    if [[ -n "${BIN_EXTENSIONS[$PLATFORM]}" ]]; then
       BIN="${TARGET}/release/m3u-filter${BIN_EXTENSIONS[$PLATFORM]}"
    else
       BIN="${TARGET}/release/m3u-filter"
    fi

    rustup target add "$TARGET"

    # Build for each platform
    cd "$WORKING_DIR"
    cargo clean || true # Clean before each build to avoid conflicts
    env RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target "$TARGET"

    # Create directories and copy binaries and config files
    cd target
    mkdir -p "$DIR"
    cp "$BIN" "$DIR"
    cp ../config/*.yml "$DIR"
    cp -rf "${FRONTEND_DIR}/build" "$DIR"/web

    # Create archive for the platform
    if [[ $PLATFORM == "WINDOWS" ]]; then
        zip -r "$ARC" "$DIR"
    else
        tar cvzf "$ARC" "$DIR"
    fi

    CHECKSUM_FILE="checksum_${ARC}.txt"
    shasum -a 256 "$ARC" >> "$CHECKSUM_FILE"

    # Move the archive and checksum to the release folder
    RELEASE_PKG="$RELEASE_DIR/release_${VERSION}"
    mkdir -p "$RELEASE_PKG"
    mv "$CHECKSUM_FILE" "$ARC" "$RELEASE_PKG"
done

# Clean up the build directories
cd "$WORKING_DIR"
cargo clean

# Commit and tag release
git add .
git commit -m "release ${VERSION}"
git tag -a "$VERSION" -m "$VERSION"
git push
git push --tags

echo "Done!"
