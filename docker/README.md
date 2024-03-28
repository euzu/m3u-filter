# Build docker image
Change into the root directory and run:

```shell
docker build --rm -f docker/Dockerfile -t m3u-filter .  
```

This will build the complete project and create a docker image.

To start the container, you can use the `docker-compose.yml`
But you need to change `image: ghcr.io/euzu/m3u-filter:latest` to `image: m3u-filter`

# Manual docker image

You want to build the binary and web folder manually and create a docker image. 

To dockerize m3u-filter, you need to compile a static build.
The static build can created with `bin\build_lin_static.sh`. 
Description of static binary compiling is in the main `README.md`

Then you need to compile the frontend with `yarn build`

Change into the `docker` directory and copy all the needed files (look at the Dockerfile) into the current directory.

To create a docker image type:
`docker -f Dockerfile-manual build -t m3u-filter  .`

To start the container, you can use the `docker-compose.yml`
But you need to change `image: ghcr.io/euzu/m3u-filter:latest` to `image: m3u-filter`
