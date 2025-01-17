#!/bin/bash
set -euo pipefail
source "${HOME}/.ghcr.io"

WORKING_DIR=$(pwd)
DOCKER_DIR="${WORKING_DIR}/docker"
FRONTEND_DIR="${WORKING_DIR}/frontend"
TARGET=x86_64-unknown-linux-musl

cd "$FRONTEND_DIR" && rm -rf build && yarn  && yarn build
cd "$WORKING_DIR"

# Check if the frontend build directory exists
if [ ! -d "$FRONTEND_DIR/build" ]; then
    echo "Error: Web directory '$FRONTEND_DIR/build' does not exist."
    exit 1
fi

cargo clean
env RUSTFLAGS="--remap-path-prefix $HOME=~" cross build --release --target "$TARGET"

# Check if the binary exists
if [ ! -f "${WORKING_DIR}/target/${TARGET}/release/m3u-filter" ]; then
    echo "Error: Static binary '${WORKING_DIR}/target/${TARGET}/release/m3u-filter' does not exist."
    exit 1
fi

# Prepare Docker build context
BIN_FILE=${WORKING_DIR}/target/${TARGET}/release/m3u-filter
cp "${WORKING_DIR}/target/${TARGET}/release/m3u-filter" "${DOCKER_DIR}/"
rm -rf "${DOCKER_DIR}/web"
cp -r "${WORKING_DIR}/frontend/build" "${DOCKER_DIR}/web"

# Get the version from the binary
VERSION=$("$BIN_FILE" -V | sed 's/m3u-filter *//')
if [ -z "${VERSION}" ]; then
    echo "Error: Failed to determine the version from the binary."
    exit 1
fi

cd "${DOCKER_DIR}"
echo "Building Docker images for version ${VERSION}"
SCRATCH_IMAGE_NAME=m3u-filter
ALPINE_IMAGE_NAME=m3u-filter-alpine

# Build scratch image and tag as "latest"
docker build -f Dockerfile-manual -t ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:"${VERSION}" --target scratch-final .
docker tag ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:"${VERSION}" ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:latest

# Build alpine image and tag as "latest"
docker build -f Dockerfile-manual -t ghcr.io/euzu/${ALPINE_IMAGE_NAME}:"${VERSION}" --target alpine-final .
docker tag ghcr.io/euzu/${ALPINE_IMAGE_NAME}:"${VERSION}" ghcr.io/euzu/${ALPINE_IMAGE_NAME}:latest

echo "Logging into GitHub Container Registry..."
docker login ghcr.io -u euzu -p "${GHCR_IO_TOKEN}"

# Push scratch
docker push ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:"${VERSION}"
docker push ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:latest

# Push alpine
docker push ghcr.io/euzu/${ALPINE_IMAGE_NAME}:"${VERSION}"
docker push ghcr.io/euzu/${ALPINE_IMAGE_NAME}:latest

# Clean up
echo "Cleaning up build artifacts..."
rm -rf "${DOCKER_DIR}/web"
rm -f "${DOCKER_DIR}/m3u-filter"

echo "Docker images for version ${VERSION} have been successfully built, tagged, and pushed."
