#!/usr/bin/env bash
# Builds Horizon with Lockout for other PCs, Windows (cross-built with MinGW) or Linux, and packs it
# with the prepared resources (horizon/dev/DIST.md). Run it from the Horizon checkout's root:
#
#   cd ~/horizon && bash ~/lockout-fabric/horizon/dev/build-dist.sh --windows|--linux [RESOURCES] [--no-resources]
#
# (build-windows.sh and build-linux.sh are this with the platform filled in.)
#
# RESOURCES is the prepared resources folder, by default resources/ in the checkout's root. Writes
# dist/<platform>/ (the game, a start script, README.txt, and the resources linked in) and
# lockout-horizon-<platform>-<arch>.tar.gz holding the same, the resources as a real folder.
# --no-resources leaves them out: an archive with no Minecraft files, which each PC then prepares
# itself. Adds the runner (horizon/dev/lockout-run) to the workspace if it isn't there, and
# refreshes it from the mod. Fails, with what to do, when the checkout isn't on the mod's Horizon
# commit, the resources aren't complete, or the game needs a library its players may not have
# (Horizon's tools/check_imports.py); a Windows exe must also reserve its stack.
set -euo pipefail

LOCKOUT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HORIZON=$(pwd)
DOC=horizon/dev/DIST.md
STACK=$((64 * 1024 * 1024))

fail() { echo "error: $*" >&2; exit 1; }
usage="usage: build-dist.sh --windows|--linux [RESOURCES] [--no-resources]"

platform=
resources=resources
with_resources=1
for arg in "$@"; do
    case "$arg" in
        --windows) platform=windows ;;
        --linux) platform=linux ;;
        --no-resources) with_resources=0 ;;
        -*) fail "unknown option $arg ($usage)" ;;
        *) resources=$arg ;;
    esac
done
[ -n "$platform" ] || fail "$usage"

[ -f Cargo.toml ] && [ -d crates/horizon-client ] || fail "run this from the Horizon checkout's root (cd ~/horizon)"

pin=$(sed -n 's/^horizon_revision = "\(.*\)"/\1/p' "$LOCKOUT/horizon/lockout/horizon-mixin.toml")
head=$(git rev-parse HEAD)
[ "$head" = "$pin" ] || fail "the checkout is on ${head:0:7}, the mod pins ${pin:0:7}: see \"A new Lockout version\" in $DOC"

if [ "$platform" = windows ]; then
    TARGET=x86_64-pc-windows-gnu
    ARCHIVE=lockout-horizon-windows-x64.tar.gz
    GAME=horizon-client.exe
    START=start.bat
    command -v x86_64-w64-mingw32-gcc > /dev/null || fail "no MinGW linker: sudo apt install mingw-w64 (Arch: sudo pacman -S mingw-w64-gcc)"
    rustup target list --installed | grep -qx "$TARGET" || fail "no Windows target: rustup target add $TARGET"
else
    [ "$(uname -s)" = Linux ] || fail "a Linux build is made on Linux"
    case "$(uname -m)" in
        x86_64) arch=x64 ;;
        aarch64) arch=arm64 ;;
        *) arch=$(uname -m) ;;
    esac
    ARCHIVE=lockout-horizon-linux-$arch.tar.gz
    GAME=horizon-client
    START=start.sh
fi
DIST=dist/$platform

# The runner (horizon/dev/lockout-run): wherever the workspace has it, else added as lockout-run/,
# and written fresh from the mod each time, its paths pointing at this checkout and this clone.
member=$(python3 -c '
import sys, tomllib
members = tomllib.load(open("Cargo.toml", "rb")).get("workspace", {}).get("members", [])
print(next((m for m in members if m.rstrip("/").split("/")[-1] == "lockout-run"), ""))')
if [ -z "$member" ]; then
    member=lockout-run
    sed -i 's/^members = \[/members = [\n    "lockout-run",/' Cargo.toml
    grep -q '"lockout-run"' Cargo.toml || fail "couldn't add lockout-run to the workspace members in Cargo.toml: add it by hand"
    echo "Added lockout-run to the workspace (undo with: git checkout -- Cargo.toml && rm -rf lockout-run)."
fi
runner=$HORIZON/$member
mkdir -p "$runner"
cp "$LOCKOUT/horizon/dev/lockout-run/main.rs" "$LOCKOUT/horizon/dev/lockout-run/build.rs" "$runner/"
python3 - "$LOCKOUT/horizon/dev/lockout-run/Cargo.toml" "$runner" "$HORIZON/crates" "$LOCKOUT/horizon/lockout/api/src/lib.rs" <<'EOF'
import os, re, sys
source, runner, crates, lib = sys.argv[1:]
text = open(source).read()
text = re.sub(r'^path = ".*lib\.rs"$', f'path = "{os.path.relpath(lib, runner)}"', text, flags=re.M)
text = text.replace('"../crates/', f'"{os.path.relpath(crates, runner)}/')
open(os.path.join(runner, "Cargo.toml"), "w").write(text)
EOF

# Before the long build: the resources must be there and current for this Horizon commit.
if [ "$with_resources" = 1 ]; then
    [ -d "$resources" ] || fail "no resources at $resources: prepare them ($DOC), pass their folder, or --no-resources"
    resources=$(cd "$resources" && pwd)
    cargo run -q --release -p horizon-prepare -- --check --output "$resources" \
        || fail "the resources at $resources aren't complete for this Horizon: it printed what to run"
fi

version=$(sed -n 's/^pub const LOCKOUT_VERSION: &str = "\(.*\)";/\1/p' "$LOCKOUT/horizon/lockout/api/src/wire.rs")
echo "Building Lockout $version on Horizon ${pin:0:7} for $platform..."

target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
if [ "$platform" = windows ]; then
    # crt-static: the C runtime, libgcc and winpthread go into the exe, not beside it.
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static" \
        cargo build --release --target "$TARGET" -p lockout-run
    built=$target/$TARGET/release/lockout-run.exe
else
    # The host's own build, so it shares the cache of `cargo run --release -p lockout-run`.
    cargo build --release -p lockout-run
    built=$target/release/lockout-run
fi

rm -rf "$DIST" && mkdir -p "$DIST"
cp "$built" "$DIST/$GAME"

# Only libraries every player has (Horizon's own release check).
python3 tools/check_imports.py "$DIST/$GAME"
if [ "$platform" = windows ]; then
    # And the stack lockout-run/build.rs asks for.
    python3 - "$DIST/$GAME" "$STACK" <<'EOF'
import struct, sys
data = open(sys.argv[1], "rb").read()
pe = struct.unpack_from("<I", data, 0x3C)[0]
optional = pe + 24
if struct.unpack_from("<H", data, optional)[0] != 0x20B:
    sys.exit(f"{sys.argv[1]}: not a 64-bit Windows program")
reserve = struct.unpack_from("<Q", data, optional + 72)[0]
if reserve < int(sys.argv[2]):
    sys.exit(f"{sys.argv[1]}: reserves a {reserve // 1024} KiB stack, needs {int(sys.argv[2]) // 1024} KiB")
print(f"{sys.argv[1]}: {reserve // (1024 * 1024)} MiB stack")
EOF
    printf '%s\r\n' '@echo off' 'cd /d "%~dp0"' 'horizon-client.exe --resources .\resources %*' > "$DIST/$START"
    system="64-bit Windows"
    unpack="Unpack this folder anywhere (Command Prompt or PowerShell: tar -xf $ARCHIVE; Windows 11's
Explorer opens it too) and start the game with start.bat."
    prepare="horizon-prepare.exe --prepare --output .\\resources --server-jar <server.jar>
  horizon-prepare.exe sounds --output .\\resources"
else
    # The newest glibc it needs: the distributions it runs on are those with that one or newer.
    glibc=$(objdump -T "$DIST/$GAME" 2>/dev/null | grep -o 'GLIBC_[0-9][0-9.]*' | sort -uV | tail -1 | cut -d_ -f2 || true)
    [ -n "$glibc" ] && echo "$DIST/$GAME: needs glibc $glibc or newer"
    printf '%s\n' '#!/bin/sh' 'cd "$(dirname "$0")" && exec ./horizon-client --resources ./resources "$@"' > "$DIST/$START"
    chmod 755 "$DIST/$START" "$DIST/$GAME"
    system="Linux ($(uname -m)${glibc:+, glibc $glibc or newer: check with ldd --version})"
    unpack="Unpack it anywhere (tar -xf $ARCHIVE) and start the game with ./start.sh. It needs a
Vulkan driver, ALSA (libasound2) and udev, which desktop distributions have."
    prepare="./horizon-prepare --prepare --output ./resources --server-jar <server.jar>
  ./horizon-prepare sounds --output ./resources"
fi

files=("$GAME" "$START" README.txt)
if [ "$with_resources" = 1 ]; then
    ln -s "$resources" "$DIST/resources"
    files+=(resources)
    cat > "$DIST/README.txt" <<EOF
Horizon with Lockout $version (Horizon ${pin:0:7}), for $system.

$unpack

resources/ holds Minecraft 26.3's files, prepared from the copy of the person who built this.
Play it only with a Minecraft: Java Edition of your own, and don't pass it on any further.

The server and every player must run Lockout $version (Fabric or Horizon).
EOF
else
    cat > "$DIST/README.txt" <<EOF
Horizon with Lockout $version (Horizon ${pin:0:7}), for $system.

No Minecraft files are in here. Each PC prepares its own from its own Minecraft 26.3 install,
while it has internet:

- with the Horizon launcher: horizon-launcher install latest, horizon-launcher prepare, then
  horizon-launcher import-build $ARCHIVE --trust-unsigned
- or without it, with horizon-prepare from a Horizon release, in this folder:
  $prepare
  then start the game with $START.

The server and every player must run Lockout $version (Fabric or Horizon).
EOF
fi
[ "$platform" = windows ] && sed -i 's/$/\r/' "$DIST/README.txt"

# -h: the resources go in as the folder they link to; --hard-dereference: every file in full, no
# links for the other PC to make.
rm -f "$ARCHIVE"
tar -czhf "$ARCHIVE" --hard-dereference -C "$DIST" "${files[@]}"
echo "Wrote $HORIZON/$ARCHIVE ($(du -h "$ARCHIVE" | cut -f1)). Test what went into it:"
res=$([ "$with_resources" = 1 ] && echo "$DIST/resources/" || echo "<your resources>/")
if [ "$platform" = windows ]; then
    echo "  wine $DIST/$GAME --resources $res"
else
    echo "  $DIST/$GAME --resources $res"
fi
