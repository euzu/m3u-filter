# Build rust stage
FROM rust:bookworm as rust-build

RUN apt-get update && apt-get install -y --no-install-recommends pkg-config musl-tools libssl-dev

# Update Rust toolchain and add necessary target
RUN rustup update && rustup target add x86_64-unknown-linux-musl

# Set Rust compiler flags for better optimization and reproducibility
ENV RUSTFLAGS='--remap-path-prefix $HOME=~ -C target-feature=+crt-static'

# Copy the actual source code and build the project
WORKDIR /src
COPY . /src
RUN cargo build --target x86_64-unknown-linux-musl --release

# Build node stage
FROM node:lts as node-build

ENV NODE_OPTIONS=--openssl-legacy-provider

COPY ./frontend /app
WORKDIR /app

RUN yarn install --prefer-offline --frozen-lockfile --non-interactive --production=false && \
    yarn build &&  rm -rf node_modules

# Build resource stage (using ffmpeg)
FROM jrottenberg/ffmpeg:latest as resource-build
WORKDIR /src
COPY resources ./resources
RUN ffmpeg -loop 1 -i ./resources/channel_unavailable.jpg -t 10 -r 1 -an \
  -vf "scale=1920:1080" \
  -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p \
  ./resources/channel_unavailable.ts

RUN ffmpeg -loop 1 -i ./resources/user_connections_exhausted.jpg -t 10 -r 1 -an \
  -vf "scale=1920:1080" \
  -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p \
  ./resources/user_connections_exhausted.ts
  
RUN ffmpeg -loop 1 -i ./resources/provider_connections_exhausted.jpg -t 10 -r 1 -an \
  -vf "scale=1920:1080" \
  -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p \
  ./resources/provider_connections_exhausted.ts

# Final container (Scratch)
FROM scratch as scratch-final
WORKDIR /app
COPY --from=rust-build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=rust-build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=rust-build /src/target/x86_64-unknown-linux-musl/release/m3u-filter ./m3u-filter
COPY --from=node-build /app/build ./web
COPY --from=resource-build /src/resources ./resources
ENTRYPOINT ["/m3u-filter"]
CMD ["-s", "-p", "/app/config"]

# Final container (Alpine)
FROM alpine:latest as alpine-final
RUN apk add --no-cache bash curl ca-certificates tini
WORKDIR /app
COPY --from=rust-build /src/target/x86_64-unknown-linux-musl/release/m3u-filter ./m3u-filter
COPY --from=node-build /app/build ./web
COPY --from=resource-build /src/resources ./resources
ENTRYPOINT ["/sbin/tini", "--", "/app/m3u-filter"]
CMD ["-s", "-p", "/app/config"]
