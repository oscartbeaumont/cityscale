#!/bin/bash
# Helper script for building and running the Docker container.
#
# `./dev.sh` - Build the Docker container.
# `./dev.sh run` - Build and run the Docker container.
#

set -e

DOCKER_TAG="${DOCKER_TAG:-oscartbeaumont/cityscale:latest}"

pnpm -C web build

if [ "$(uname)" == "Linux" ]; then
    cargo build --release
else
    cargo zigbuild --target x86_64-unknown-linux-gnu --release
fi

cp ./target/x86_64-unknown-linux-gnu/release/cityscale ./docker/cityscale
docker build --platform linux/amd64 -t "$DOCKER_TAG" ./docker

if [ "$1" == "run" ]; then
    echo "Running container..."
    docker run -it --rm --platform linux/amd64 -p 2489:2489 -p 3306:3306 "$DOCKER_TAG"
fi
