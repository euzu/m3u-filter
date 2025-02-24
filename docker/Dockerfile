# Build rust stage
FROM rust:bookworm as rust-build

RUN apt-get update && \
  apt-get install pkg-config musl-tools libssl-dev

RUN rustup update
ENV RUSTFLAGS='--remap-path-prefix $HOME=~ -C target-feature=+crt-static'

COPY . /src
WORKDIR /src

RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --target x86_64-unknown-linux-musl --release

# Build node stage
FROM node:lts as node-build

ENV NODE_OPTIONS=--openssl-legacy-provider

COPY ./frontend /app
WORKDIR /app

RUN yarn install \
  --prefer-offline \
  --frozen-lockfile \
  --non-interactive \
  --production=false
RUN yarn build
RUN rm -rf node_modules

# Build resource stage (using ffmpeg)
FROM jrottenberg/ffmpeg:latest as resource-build
WORKDIR /src
COPY resources ./resources
RUN ffmpeg -loop 1 -i ./resources/freeze_frame.jpg -t 10 -r 1 -an \
  -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p \
  ./resources/freeze_frame.ts

# Final container (Scratch)
FROM scratch as scratch-final
WORKDIR /
COPY --from=rust-build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=rust-build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=rust-build /src/target/x86_64-unknown-linux-musl/release/m3u-filter /m3u-filter
COPY --from=node-build /app/build /web
COPY --from=resource-build /src/resources/freeze_frame.ts /freeze_frame.ts
ENTRYPOINT ["/m3u-filter"]
CMD ["-s", "-p", "/config"]

# Final container (Alpine)
FROM alpine:latest as alpine-final
RUN apk add --no-cache bash curl ca-certificates tini
WORKDIR /app
COPY --from=rust-build /src/target/x86_64-unknown-linux-musl/release/m3u-filter m3u-filter
COPY --from=node-build /app/build web
COPY --from=resource-build /src/resources/freeze_frame.ts /freeze_frame.ts
ENTRYPOINT ["/sbin/tini", "--", "/app/m3u-filter"]
CMD ["-s", "-p", "/app/config"]
