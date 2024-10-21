#!/bin/bash
set -euo pipefail

source "${HOME}/.ghcr.io"

# Check if the binary exists
if [ ! -f "./target/x86_64-unknown-linux-musl/release/m3u-filter" ]; then
    echo "Error: Static binary '../target/x86_64-unknown-linux-musl/release/m3u-filter' does not exist."
    exit 1
fi

# Check if the frontend build directory exists
if [ ! -d "./frontend/build" ]; then
    echo "Error: Web directory '../frontend/build' does not exist."
    exit 1
fi

# Prepare Docker build context
cd ./docker
cp ../target/x86_64-unknown-linux-musl/release/m3u-filter .
rm -rf ./web
cp -r ../frontend/build ./web

# Get the version from the binary
VERSION=$(./m3u-filter -V | sed 's/m3u-filter *//')
if [ -z "${VERSION}" ]; then
    echo "Error: Failed to determine the version from the binary."
    exit 1
fi

echo "Building Docker images for version ${VERSION}"

# Build scratch image and tag as "latest"
docker build -f Dockerfile-manual -t ghcr.io/euzu/m3u-filter:"${VERSION}" --target scratch-final .
docker tag ghcr.io/euzu/m3u-filter:"${VERSION}" ghcr.io/euzu/m3u-filter:latest

# Build alpine image and tag as "latest"
docker build -f Dockerfile-manual -t ghcr.io/euzu/m3u-filter-alpine:"${VERSION}" --target alpine-final .
docker tag ghcr.io/euzu/m3u-filter-alpine:"${VERSION}" ghcr.io/euzu/m3u-filter-alpine:latest

echo "Logging into GitHub Container Registry..."
docker login ghcr.io -u euzu -p "${GHCR_IO_TOKEN}"

# Push scratch
docker push ghcr.io/euzu/m3u-filter:"${VERSION}"
docker push ghcr.io/euzu/m3u-filter:latest

# Push alpine
docker push ghcr.io/euzu/m3u-filter-alpine:"${VERSION}"
docker push ghcr.io/euzu/m3u-filter-alpine:latest

# Clean up
echo "Cleaning up build artifacts..."
rm -rf ./web
rm -f ./m3u-filter

echo "Docker images for version ${VERSION} have been successfully built, tagged, and pushed."
