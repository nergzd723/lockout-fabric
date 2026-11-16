#!/usr/bin/env bash
# build-dist.sh for Linux (horizon/dev/DIST.md): cd ~/horizon && bash ~/lockout-fabric/horizon/dev/build-linux.sh
exec bash "$(dirname "${BASH_SOURCE[0]}")/build-dist.sh" --linux "$@"
