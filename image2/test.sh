#!/bin/bash

set -e

docker build -t oscartbeaumont/cityscale:latest .

# TODO: Data volume + ports
docker run --rm --platform linux/amd64 oscartbeaumont/cityscale:latest
