#!/usr/bin/env bash
# Builds Horizon with Lockout for other machines, Windows (cross-built with MinGW), Linux (this
# machine's own kind) or Linux on 64-bit ARM (cross-built, or native on an ARM machine), and packs it
# with the prepared resources (horizon/dev/DIST.md). Run it from the Horizon checkout's root:
#
#   cd ~/horizon && bash ~/lockout-fabric/horizon/dev/build-dist.sh --windows|--linux|--linux-arm64 [--server] [RESOURCES] [--no-resources]
#
# (build-windows.sh, build-linux.sh, build-linux-arm64.sh and build-server.sh are this with the
# platform filled in.)
#
# RESOURCES is the prepared resources folder, by default resources/ in the checkout's root. Writes
# dist/<platform>/ (the game, a start script, README.txt, and the resources linked in) and
# lockout-horizon-<platform>-<arch>.tar.gz holding the same, the resources as a real folder.
# --server builds a dedicated server instead (Linux): horizon-server, Horizon's server supervisor,
# and horizon-engine, the game without sound or controllers with Lockout in it, in
# lockout-horizon-server-<platform>-<arch>.tar.gz, to unpack in the server's folder. With
# cargo-zigbuild installed (and Zig), both are static (musl), for any Linux of that CPU.
# --no-resources leaves the resources out: an archive with no Minecraft files, which each machine then
# prepares itself. Adds the runner (horizon/dev/lockout-run) to the workspace if it isn't there, and
# refreshes it from the mod. Fails, with what to do, when the checkout isn't on the mod's Horizon
# commit, the resources aren't complete, or a program needs a library its machine may not have
# (Horizon's tools/check_imports.py); a Windows exe must also reserve its stack.
set -euo pipefail

LOCKOUT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HORIZON=$(pwd)
DOC=horizon/dev/DIST.md
STACK=$((64 * 1024 * 1024))

fail() { echo "error: $*" >&2; exit 1; }
usage="usage: build-dist.sh --windows|--linux|--linux-arm64 [--server] [RESOURCES] [--no-resources]"

platform=
server=0
resources=resources
with_resources=1
for arg in "$@"; do
    case "$arg" in
        --windows) platform=windows ;;
        --linux) platform=linux ;;
        --linux-arm64) platform=linux-arm64 ;;
        --server) server=1 ;;
        --no-resources) with_resources=0 ;;
        -*) fail "unknown option $arg ($usage)" ;;
        *) resources=$arg ;;
    esac
done
[ -n "$platform" ] || fail "$usage"
[ "$server" = 1 ] && [ "$platform" = windows ] && fail "--server builds a Linux server: --linux or --linux-arm64"

[ -f Cargo.toml ] && [ -d crates/horizon-client ] || fail "run this from the Horizon checkout's root (cd ~/horizon)"

pin=$(sed -n 's/^horizon_revision = "\(.*\)"/\1/p' "$LOCKOUT/horizon/lockout/horizon-mixin.toml")
head=$(git rev-parse HEAD)
[ "$head" = "$pin" ] || fail "the checkout is on ${head:0:7}, the mod pins ${pin:0:7}: see \"A new Lockout version\" in $DOC"

TARGET=
zig=0
native=1
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
    GAME=horizon-client
    START=start.sh
    # A server build is static when cargo-zigbuild is installed: Zig compiles the C parts and links
    # musl in, so the programs need no libc on the server at all and run on any Linux of that CPU,
    # and ARM needs no cross compiler. (The game can't be: it loads the GPU's libraries at run time,
    # which a static program can't.) Horizon's mimalloc stays the allocator, not musl's.
    if [ "$server" = 1 ] && cargo zigbuild --help > /dev/null 2>&1; then
        zig=1
        TARGET=$([ "$platform" = linux-arm64 ] && echo aarch64 || uname -m)-unknown-linux-musl
        rustup target list --installed | grep -qx "$TARGET" || fail "no musl target: rustup target add $TARGET"
    fi
    if [ "$platform" = linux-arm64 ] && [ "$arch" != arm64 ]; then
        native=0
        if [ "$zig" = 0 ]; then
            TARGET=aarch64-unknown-linux-gnu
            rustup target list --installed | grep -qx "$TARGET" || fail "no ARM target: rustup target add $TARGET"
            # Cross-built: Debian or Ubuntu's arm64 compiler, and (for the game, which has sound
            # and controllers) their arm64 ALSA and udev to link with.
            command -v aarch64-linux-gnu-gcc > /dev/null \
                || fail "no ARM compiler: install cargo-zigbuild for a server, or sudo apt install gcc-aarch64-linux-gnu (see \"Linux on ARM\" in $DOC)"
            if [ "$server" = 0 ]; then
                for pc in alsa libudev; do
                    [ -f "/usr/lib/aarch64-linux-gnu/pkgconfig/$pc.pc" ] \
                        || fail "no arm64 $pc to link with: sudo apt install libasound2-dev:arm64 libudev-dev:arm64 (see \"Linux on ARM\" in $DOC)"
                done
            fi
        fi
        arch=arm64
    fi
    if [ "$server" = 1 ]; then
        ARCHIVE=lockout-horizon-server-linux-$arch.tar.gz
    else
        ARCHIVE=lockout-horizon-linux-$arch.tar.gz
    fi
fi
DIST=dist/$([ "$server" = 1 ] && echo "server-$platform" || echo "$platform")

# The runner (horizon/dev/lockout-run): wherever the workspace has it, else added as lockout-run/,
# and written fresh from the mod each time, its paths pointing at this checkout and this clone.
# (Read without tomllib, which Ubuntu 22.04's Python 3.10 lacks.)
member=$(python3 - <<'EOF'
import re
block = re.search(r"^members\s*=\s*\[(.*?)\]", open("Cargo.toml").read(), re.M | re.S)
members = re.findall(r'"([^"]+)"', block.group(1)) if block else []
print(next((m for m in members if m.rstrip("/").split("/")[-1] == "lockout-run"), ""))
EOF
)
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
echo "Building Lockout $version on Horizon ${pin:0:7} for $platform$([ "$server" = 1 ] && echo ", the server")..."

# cargo build --release for the platform, with its linker settings.
build() {
    if [ "$platform" = windows ]; then
        # crt-static: the C runtime, libgcc and winpthread go into the exe, not beside it.
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static" \
            cargo build --release --target "$TARGET" "$@"
    elif [ "$zig" = 1 ]; then
        cargo zigbuild --release --target "$TARGET" "$@"
    elif [ -n "$TARGET" ]; then
        # pkg-config reads the arm64 .pc files, which point the linker at the arm64 libraries.
        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
        PKG_CONFIG_ALLOW_CROSS=1 \
        PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig \
            cargo build --release --target "$TARGET" "$@"
    else
        # The host's own build, so it shares the cache of `cargo run --release -p lockout-run`.
        cargo build --release "$@"
    fi
}
target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
out=$target${TARGET:+/$TARGET}/release

rm -rf "$DIST" && mkdir -p "$DIST"
if [ "$server" = 1 ]; then
    # The engine as Horizon's server release builds it (--no-default-features: no sound or
    # controllers), and the supervisor from the same commit, so the two speak the same protocol.
    build -p lockout-run --no-default-features
    cp "$out/lockout-run" "$DIST/horizon-engine"
    build -p horizon-server
    cp "$out/horizon-server" "$DIST/horizon-server"
    programs=(horizon-engine horizon-server)
else
    build -p lockout-run
    cp "$out/lockout-run$([ "$platform" = windows ] && echo .exe)" "$DIST/$GAME"
    programs=("$GAME")
fi

# Only libraries every player has: Horizon's own release check (--server: not even ALSA or udev),
# which knows x86-64's loader, so an arm64 program is checked against arm64's.
check=()
[ "$server" = 1 ] && check+=(--server)
if [ "$zig" = 1 ]; then
    # Static: no loader and no library at all (read with the same check's ELF reader).
    for p in "${programs[@]}"; do
        python3 - "$DIST/$p" <<'EOF'
import sys
sys.path.insert(0, "tools")
import check_imports
interpreter, needed, _ = check_imports.elf_needs(open(sys.argv[1], "rb").read())
if interpreter or needed:
    sys.exit(f"{sys.argv[1]}: not static: loader {interpreter}, needs {', '.join(needed)}")
print(f"{sys.argv[1]}: static, needs no library")
EOF
    done
elif [ "$platform" = linux-arm64 ]; then
    for p in "${programs[@]}"; do
        python3 - "$DIST/$p" "$server" <<'EOF'
import sys
sys.path.insert(0, "tools")
import check_imports
check_imports.LINUX_LOADER = "/lib/ld-linux-aarch64.so.1"
check_imports.LINUX_SYSTEM = (check_imports.LINUX_SYSTEM - {"ld-linux-x86-64.so.2"}) | {"ld-linux-aarch64.so.1"}
sys.exit(0 if check_imports.check(sys.argv[1], server=sys.argv[2] == "1") else 1)
EOF
    done
else
    python3 tools/check_imports.py "${check[@]}" "${programs[@]/#/$DIST/}"
fi
if [ "$server" = 1 ] && [ "$native" = 1 ]; then
    # As Horizon's release checks its engine: it starts and knows its options.
    "$DIST/horizon-engine" --help | grep -q -- '--resources' || fail "$DIST/horizon-engine doesn't start"
fi

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
    # The newest glibc they need (the version names their dynamic section lists, any CPU's): the
    # distributions they run on are those with that one or newer.
    glibc=
    if [ "$zig" = 0 ]; then
        glibc=$(for p in "${programs[@]}"; do grep -ao 'GLIBC_2\.[0-9][0-9.]*' "$DIST/$p" || true; done \
            | cut -d_ -f2 | sort -uV | tail -1)
        [ -n "$glibc" ] && echo "$DIST: needs glibc $glibc or newer"
    fi
    chmod 755 "${programs[@]/#/$DIST/}"
    cpu=$([ "$arch" = arm64 ] && echo "64-bit ARM" || echo "$(uname -m)")
    system="Linux ($cpu${glibc:+, glibc $glibc or newer: check with ldd --version})"
    [ "$zig" = 1 ] && system="Linux ($cpu, static: any distribution)"
    if [ "$server" = 0 ]; then
        printf '%s\n' '#!/bin/sh' 'cd "$(dirname "$0")" && exec ./horizon-client --resources ./resources "$@"' > "$DIST/$START"
        chmod 755 "$DIST/$START"
        programs+=("$START")
    fi
    unpack="Unpack it anywhere (tar -xf $ARCHIVE) and start the game with ./start.sh. It needs a
Vulkan driver, ALSA (libasound2) and udev, which desktop distributions have."
    prepare="./horizon-prepare --prepare --output ./resources --server-jar <server.jar>
  ./horizon-prepare sounds --output ./resources"
fi

files=("${programs[@]}" README.txt)
[ "$with_resources" = 1 ] && ln -s "$resources" "$DIST/resources" && files+=(resources)
if [ "$server" = 1 ]; then
    cat > "$DIST/README.txt" <<EOF
Horizon's dedicated server with Lockout $version (Horizon ${pin:0:7}), for $system.

horizon-server runs the server; horizon-engine is the game it runs, with Lockout. Unpack them into the
server's folder (a vanilla or Fabric server's folder works as it is: its world, server.properties, ops,
whitelist and Lockout's saved match are kept):

  tar -xf $ARCHIVE

Then once, to agree to the EULA and write server.toml (it imports server.properties):

  ./horizon-server init

and to play: ./horizon-server run (the console on this terminal; Ctrl-C stops the server), or
./horizon-server start, status and stop to run it in the background.

$([ "$with_resources" = 1 ] && echo "resources/ holds Minecraft 26.3's files, prepared from your copy; the server reads them from there." || echo "The server needs Minecraft 26.3's files, prepared on a computer with the game (horizon-prepare): copy
them here with ./horizon-server import-resources <folder>.")

Every player needs Lockout $version (Fabric or Horizon). Settings: server.toml (docs/hosting.md in Horizon).
EOF
elif [ "$with_resources" = 1 ]; then
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
# links for the other machine to make.
rm -f "$ARCHIVE"
tar -czhf "$ARCHIVE" --hard-dereference -C "$DIST" "${files[@]}"
echo "Wrote $HORIZON/$ARCHIVE ($(du -h "$ARCHIVE" | cut -f1)). Test what went into it:"
res=$([ "$with_resources" = 1 ] && echo "$DIST/resources/" || echo "<your resources>/")
if [ "$server" = 1 ]; then
    echo "  unpack it in a copy of a server folder, then ./horizon-server init and ./horizon-server run"
elif [ "$platform" = windows ]; then
    echo "  wine $DIST/$GAME --resources $res"
else
    echo "  $DIST/$GAME --resources $res"
fi
