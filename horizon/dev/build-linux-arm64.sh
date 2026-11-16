#!/usr/bin/env bash
# build-dist.sh for Linux on 64-bit ARM (horizon/dev/DIST.md, "Linux on ARM"):
# cd ~/horizon && bash ~/lockout-fabric/horizon/dev/build-linux-arm64.sh
exec bash "$(dirname "${BASH_SOURCE[0]}")/build-dist.sh" --linux-arm64 "$@"
