FROM gcr.io/distroless/base-debian12 as build


FROM scratch
WORKDIR /

COPY --from=build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

COPY ./m3u-filter /
COPY ./web /web

CMD ["./m3u-filter", "-s", "-p", "/config"]
