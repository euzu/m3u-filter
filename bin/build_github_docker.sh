#!/bin/bash
source "${HOME}"/.ghcr.io
cd ./docker && cp ../target/x86_64-unknown-linux-musl/release/m3u-filter . && rm -rf ./web && cp -r ../frontend/build ./web

if [ $? -eq 0 ]; then
  VERSION=$(./m3u-filter -V | sed 's/m3u-filter *//')
  if [ -n "${VERSION}" ]; then
    echo "building docker image version ${VERSION}"
    docker build -f Dockerfile-manual -t ghcr.io/euzu/m3u-filter:"${VERSION}" . && \
    docker tag ghcr.io/euzu/m3u-filter:"${VERSION}" ghcr.io/euzu/m3u-filter:latest  && \
    docker login ghcr.io -u euzu -p "${GHCR_IO_TOKEN}" && \
    docker push ghcr.io/euzu/m3u-filter:"${VERSION}" && \
    docker push ghcr.io/euzu/m3u-filter:latest && \
    rm -rf ./web && \
    rm -f ./m3u-filter
  fi
fi
