#!/usr/bin/env bash

docker buildx build --platform linux/amd64,linux/amd64/v2,linux/amd64/v3,linux/arm64,linux/arm/v7 -t gordlea/fireboard2mqtt:latest -f ./docker/standalone.Dockerfile .

