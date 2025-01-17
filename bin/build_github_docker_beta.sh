#!/bin/bash
set -euo pipefail

source "${HOME}/.ghcr.io"

TARGET=x86_64-unknown-linux-musl

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

# Prepare Docker build context
cd ./docker
cp "../target/${TARGET}/release/m3u-filter" .
rm -rf ./web
cp -r ../frontend/build ./web

# Get the version from the binary
VERSION=$(./m3u-filter -V | sed 's/m3u-filter *//')
if [ -z "${VERSION}" ]; then
    echo "Error: Failed to determine the version from the binary."
    exit 1
fi

# Split the version into its components using '.' as a delimiter
IFS='.' read -r major minor patch <<< "$VERSION"

# Increment the patch version
patch=$((patch + 1))

# Combine the components back into a version string
VERSION="$major.$minor.${patch}-beta"

echo "Building Docker images for version ${VERSION}"

# Build beta image and tag as "latest"
docker build -f Dockerfile-manual -t ghcr.io/euzu/m3u-filter-beta:"${VERSION}" --target scratch-final .
docker tag ghcr.io/euzu/m3u-filter-beta:"${VERSION}" ghcr.io/euzu/m3u-filter-beta:latest

echo "Logging into GitHub Container Registry..."
docker login ghcr.io -u euzu -p "${GHCR_IO_TOKEN}"

# Push beta
docker push ghcr.io/euzu/m3u-filter-beta:"${VERSION}"
docker push ghcr.io/euzu/m3u-filter-beta:latest

# Clean up
echo "Cleaning up build artifacts..."
rm -rf ./web
rm -f ./m3u-filter

echo "Docker images for version ${VERSION} have been successfully built, tagged, and pushed."
