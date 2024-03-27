# Build rust
FROM rust:bookworm as rust-build

RUN apt-get update && \
	apt-get install pkg-config musl-tools libssl-dev

RUN rustup update
ENV RUSTFLAGS='-C target-feature=+crt-static'

COPY . /src
WORKDIR /src

RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --target x86_64-unknown-linux-musl --release


# Final container
FROM scratch
WORKDIR /

COPY --from=rust-build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=rust-build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

COPY --from=rust-build /src/target/x86_64-unknown-linux-musl/release/m3u-filter /m3u-filter
COPY ./config /config

ENTRYPOINT ["/m3u-filter"]
CMD ["-s", "-p", "/config"]
