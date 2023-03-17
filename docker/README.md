# Dockerize
To dockerize m3u-filter, you need to compile a static build.
The static build can creted with `bin\build_lin_static.sh`. 
Description of static binary compiling is in the main `README.md`

To create a docker image type:
`docker build -t m3u-filter  .`
You have to copy all the needed files (look at the Dockerfile) into the current directory.

To start the container, you can use the `docker-compose.yml`
