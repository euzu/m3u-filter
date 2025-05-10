# Build docker image

Targets are
- `scratch-final`
- `alpine-final`

Change into the root directory and run:

```shell
# Build for a specific architecture
docker build --rm -f docker/Dockerfile -t tuliprox --target scratch-final --build-arg RUST_TARGET=x86_64-unknown-linux-musl .

docker build --rm -f docker/Dockerfile -t tuliprox --target scratch-final --build-arg RUST_TARGET=aarch64-unknown-linux-musl .

docker build --rm -f docker/Dockerfile -t tuliprox --target scratch-final --build-arg RUST_TARGET=armv7-unknown-linux-musleabihf .

docker build --rm -f docker/Dockerfile -t tuliprox --target scratch-final --build-arg RUST_TARGET=x86_64-apple-darwin .
```
Both targets have the path prefix: `/app`  

This will build the complete project and create a docker image.

To start the container, you can use the `docker-compose.yml`
But you need to change `image: ghcr.io/euzu/tuliprox:latest` to `image: tuliprox`

# Manual docker image

You want to build the binary and web folder manually and create a docker image. 

To dockerize tuliprox, you need to compile a static build.
The static build can created with `bin\build_lin_static.sh`. 
Description of static binary compiling is in the main `README.md`

Then you need to compile the frontend with `yarn build`

Change into the `docker` directory and copy all the needed files (look at the Dockerfile) into the current directory.

To create a docker image type:
`docker -f Dockerfile-manual build -t tuliprox  .`

To start the container, you can use the `docker-compose.yml`
But you need to change `image: ghcr.io/euzu/tuliprox:latest` to `image: tuliprox`
