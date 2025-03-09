#!/bin/bash
set -euo pipefail

source "${HOME}/.ghcr.io"

WORKING_DIR=$(pwd)


TARGET=aarch64-unknown-linux-musl

(bin/build_fe.sh && bin/build_lin_static.sh $TARGET && bin/build_resources.sh ) || exit 1;

# Check if the binary exists
if [ ! -f "./target/${TARGET}/release/m3u-filter" ]; then
    echo "Error: Static binary '../target/${TARGET}/release/m3u-filter' does not exist."
    exit 1
fi

# Check if the frontend build directory exists
if [ ! -d "./frontend/build" ]; then
    echo "Error: Web directory '../frontend/build' does not exist."
    exit 1
fi

if [ ! -f "./resources/freeze_frame.ts" ]; then
    echo "Error: ./resources/freeze_frame.ts does not exist."
    exit 1
fi

# Prepare Docker build context
cd ./docker
cp "../target/${TARGET}/release/m3u-filter" .
rm -rf ./web
cp -r ../frontend/build ./web
cp -r ../resources/freeze_frame.ts .

VERSION=$(grep '^version =' "${WORKING_DIR}/Cargo.toml" | sed -E 's/version = "(.*)"/\1/')
if [ -z "${VERSION}" ]; then
    echo "Error: Failed to determine the version."
    exit 1
fi

# Split the version into its components using '.' as a delimiter
IFS='.' read -r major minor patch <<< "$VERSION"

# Increment the patch version
patch=$((patch + 1))

# Combine the components back into a version string
VERSION="$major.$minor.${patch}-beta"

IMAGE_NAME=m3u-filter-aarch64-beta

echo "Building Docker images for version ${VERSION}"

# Build beta image and tag as "latest"
docker build -f Dockerfile-manual -t ghcr.io/euzu/${IMAGE_NAME}:"${VERSION}" --target scratch-final .
docker tag ghcr.io/euzu/${IMAGE_NAME}:"${VERSION}" ghcr.io/euzu/${IMAGE_NAME}:latest

echo "Logging into GitHub Container Registry..."
docker login ghcr.io -u euzu -p "${GHCR_IO_TOKEN}"

# Push beta
docker push ghcr.io/euzu/${IMAGE_NAME}:"${VERSION}"
docker push ghcr.io/euzu/${IMAGE_NAME}:latest

# Clean up
echo "Cleaning up build artifacts..."
rm -rf ./web
rm -f ./m3u-filter
rm -f ./freeze_frame.ts

echo "Docker images for version ${VERSION} have been successfully built, tagged, and pushed."
