#!/usr/bin/env bash
# build-dist.sh for a dedicated Linux server (horizon/dev/DIST.md, "A server"): this machine's kind,
# or --linux-arm64 for an ARM one: cd ~/horizon && bash ~/lockout-fabric/horizon/dev/build-server.sh [--linux-arm64]
platform=--linux
for a in "$@"; do [ "$a" = --linux-arm64 ] && platform=; done
exec bash "$(dirname "${BASH_SOURCE[0]}")/build-dist.sh" $platform --server "$@"
