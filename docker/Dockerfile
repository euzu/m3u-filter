FROM alpine as app-builder

FROM scratch

WORKDIR /
COPY ./m3u-filter /
COPY ./config.yml /
COPY ./web /web

# the tls certificates:
COPY --from=alpine:latest /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

CMD ["./m3u-filter", "-s", "-c", "./config.yml"]
