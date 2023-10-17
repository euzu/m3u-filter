FROM alpine as app-builder

FROM scratch
COPY ./m3u-filter /
COPY ./config.yml /
COPY ./web /web
WORKDIR /
CMD ["./m3u-filter", "-s", "-c", "./config.yml"]
