#!/usr/bin/env bash
set -euo pipefail
source "${HOME}/.ghcr.io"

WORKING_DIR=$(pwd)
BIN_DIR="${WORKING_DIR}/bin"
RESOURCES_DIR="${WORKING_DIR}/resources"
DOCKER_DIR="${WORKING_DIR}/docker"
FRONTEND_DIR="${WORKING_DIR}/frontend"
TARGET=x86_64-unknown-linux-musl
VERSION=$(grep -Po '^version\s*=\s*"\K[0-9\.]+' Cargo.toml)
if [ -z "${VERSION}" ]; then
    echo "Error: Failed to determine the version from Cargo.toml."
    exit 1
fi

# Split the version into its components using '.' as a delimiter
IFS='.' read -r major minor patch <<< "$VERSION"
# Increment the patch version
patch=$((patch + 1))
# Combine the components back into a version string
VERSION="$major.$minor.${patch}-beta"

if [ ! -f "${BIN_DIR}/build_resources.sh" ]; then
  "${BIN_DIR}/build_resources.sh"
fi

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
cp "${WORKING_DIR}/target/${TARGET}/release/m3u-filter" "${DOCKER_DIR}/"
rm -rf "${DOCKER_DIR}/web"
cp -r "${WORKING_DIR}/frontend/build" "${DOCKER_DIR}/web"
cp -r "${RESOURCES_DIR}" "${DOCKER_DIR}/"

cd "${DOCKER_DIR}"
echo "Building Docker images for version ${VERSION}"
SCRATCH_IMAGE_NAME=m3u-filter-beta

# Build scratch image and tag as "latest"
docker build -f Dockerfile-manual -t ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:"${VERSION}" --target scratch-final .
docker tag ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:"${VERSION}" ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:latest

echo "Logging into GitHub Container Registry..."
docker login ghcr.io -u euzu -p "${GHCR_IO_TOKEN}"

# Push scratch
docker push ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:"${VERSION}"
docker push ghcr.io/euzu/${SCRATCH_IMAGE_NAME}:latest

# Clean up
echo "Cleaning up build artifacts..."
rm -rf "${DOCKER_DIR}/web"
rm -f "${DOCKER_DIR}/m3u-filter"
rm -rf "${DOCKER_DIR}/resources"

echo "Docker images ghcr.io/euzu/${SCRATCH_IMAGE_NAME}${VERSION} have been successfully built, tagged, and pushed."
