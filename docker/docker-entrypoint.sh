#!/usr/bin/env bash
export RUST_LOG=$(echo $RUST_LOG | xargs)
fireboard2mqtt