#!/usr/bin/env bash
# build-dist.sh for Windows (horizon/dev/DIST.md): cd ~/horizon && bash ~/lockout-fabric/horizon/dev/build-windows.sh
exec bash "$(dirname "${BASH_SOURCE[0]}")/build-dist.sh" --windows "$@"
