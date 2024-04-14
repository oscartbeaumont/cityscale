#!/bin/bash
# Helper script for building and running the Docker container.
#
# `./dev.sh` - Build the Docker container.
# `./dev.sh run` - Build and run the Docker container.
#

set -e

DOCKER_TAG="${DOCKER_TAG:-oscartbeaumont/cityscale:latest}"

pnpm -C web build

# We use `cargo-zigbuild` on Linux to avoid glibc issues.
cargo zigbuild --target x86_64-unknown-linux-gnu --release
cp ./target/x86_64-unknown-linux-gnu/release/cityscale ./docker/cityscale

docker build --platform linux/amd64 -t "$DOCKER_TAG" ./docker

if [ "$1" == "run" ]; then
    echo "Running container..."
    docker run -it --rm --platform linux/amd64 -p 2489:2489 -p 3306:3306 -e "UNSAFE_CITYSCALE_SECRET=ce33b0940b56c671666ed7ce3b00bddaafbdffdc1b95d780465efc68e93898c6be05cadb5db47dd5a1f27d4d8e7d67c0" -e "UNSAFE_CITYSCALE_MYSQL_ROOT_PASSWORD=admin" "$DOCKER_TAG"
fi
