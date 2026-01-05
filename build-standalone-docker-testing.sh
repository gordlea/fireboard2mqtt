#!/usr/bin/env bash

docker buildx build --platform linux/amd64/v2 -t gordlea/fireboard2mqtt:testing -f ./docker/standalone.Dockerfile .