#!/usr/bin/env bash
# Builds a Windows horizon-client.exe with Lockout on Linux and packs it, with the prepared
# resources, for other PCs (horizon/dev/WINDOWS.md). Run it from the Horizon checkout's root, set
# up as that page says:
#
#   cd ~/horizon && bash ~/lockout-fabric/horizon/dev/build-windows.sh [RESOURCES] [--no-resources]
#
# RESOURCES is the prepared resources folder, by default resources/ in the checkout's root. Writes
# dist/ (horizon-client.exe, start.bat, README.txt, and resources linked in) and
# lockout-horizon-windows-x64.tar.gz holding the same, the resources as a real folder.
# --no-resources leaves them out: an archive with no Minecraft files, which each PC then prepares
# itself. Fails, with what to do, when the checkout isn't on the mod's Horizon commit, the runner is
# out of date, the resources aren't complete, the exe doesn't reserve its stack, or it needs a DLL
# that isn't part of Windows.
set -euo pipefail

TARGET=x86_64-pc-windows-gnu
STACK=$((64 * 1024 * 1024))
ARCHIVE=lockout-horizon-windows-x64.tar.gz
LOCKOUT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HORIZON=$(pwd)

fail() { echo "error: $*" >&2; exit 1; }

resources=resources
with_resources=1
for arg in "$@"; do
    case "$arg" in
        --no-resources) with_resources=0 ;;
        -*) fail "unknown option $arg (usage: build-windows.sh [RESOURCES] [--no-resources])" ;;
        *) resources=$arg ;;
    esac
done

[ -f Cargo.toml ] && [ -d crates/horizon-client ] || fail "run this from the Horizon checkout's root (cd ~/horizon)"
grep -q '"lockout-run"' Cargo.toml || fail "lockout-run isn't in the workspace: do the setup in horizon/dev/WINDOWS.md"

pin=$(sed -n 's/^horizon_revision = "\(.*\)"/\1/p' "$LOCKOUT/horizon/lockout/horizon-mixin.toml")
head=$(git rev-parse HEAD)
[ "$head" = "$pin" ] || fail "the checkout is on ${head:0:7}, the mod pins ${pin:0:7}: see \"A new Lockout version\" in horizon/dev/WINDOWS.md"

# The runner as the mod has it; its Cargo.toml may point [lib] path elsewhere.
for f in Cargo.toml main.rs build.rs; do
    cmp -s <(grep -v '^path = .*lib\.rs"' "$LOCKOUT/horizon/dev/lockout-run/$f") <(grep -sv '^path = .*lib\.rs"' "lockout-run/$f") \
        || fail "lockout-run/$f differs from the mod's: cp -r $LOCKOUT/horizon/dev/lockout-run $HORIZON/"
done

command -v x86_64-w64-mingw32-gcc > /dev/null || fail "no MinGW linker: sudo apt install mingw-w64 (Arch: sudo pacman -S mingw-w64-gcc)"
rustup target list --installed | grep -qx "$TARGET" || fail "no Windows target: rustup target add $TARGET"

# Before the long build: the resources must be there and current for this Horizon commit.
if [ "$with_resources" = 1 ]; then
    [ -d "$resources" ] || fail "no resources at $resources: prepare them (horizon/dev/WINDOWS.md), pass their folder, or --no-resources"
    resources=$(cd "$resources" && pwd)
    cargo run -q --release -p horizon-prepare -- --check --output "$resources" \
        || fail "the resources at $resources aren't complete for this Horizon: it printed what to run"
fi

version=$(sed -n 's/^pub const LOCKOUT_VERSION: &str = "\(.*\)";/\1/p' "$LOCKOUT/horizon/lockout/api/src/wire.rs")
echo "Building Lockout $version on Horizon ${pin:0:7} for Windows..."

# crt-static: the C runtime, libgcc and winpthread go into the exe, not beside it.
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static" \
    cargo build --release --target "$TARGET" -p lockout-run

target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
rm -rf dist && mkdir dist
cp "$target/$TARGET/release/lockout-run.exe" dist/horizon-client.exe

# Only Windows' own DLLs (Horizon's own release check), and the stack lockout-run/build.rs asks for.
python3 tools/check_imports.py dist/horizon-client.exe
python3 - dist/horizon-client.exe "$STACK" <<'EOF'
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

printf '%s\r\n' '@echo off' 'cd /d "%~dp0"' 'horizon-client.exe --resources .\resources %*' > dist/start.bat
files=(horizon-client.exe start.bat README.txt)
if [ "$with_resources" = 1 ]; then
    ln -s "$resources" dist/resources
    files+=(resources)
    cat > dist/README.txt <<EOF
Horizon with Lockout $version (Horizon ${pin:0:7}), for 64-bit Windows.

Unpack this folder anywhere (Command Prompt or PowerShell: tar -xf $ARCHIVE; Windows 11's
Explorer opens it too) and start the game with start.bat.

resources\\ holds Minecraft 26.3's files, prepared from the copy of the person who built this.
Play it only with a Minecraft: Java Edition of your own, and don't pass it on any further.

The server and every player must run Lockout $version (Fabric or Horizon).
EOF
else
    cat > dist/README.txt <<EOF
Horizon with Lockout $version (Horizon ${pin:0:7}), for 64-bit Windows.

No Minecraft files are in here. Each PC prepares its own from its own Minecraft 26.3 install,
while it has internet:

- with the Horizon launcher: horizon-launcher install latest, horizon-launcher prepare, then
  horizon-launcher import-build $ARCHIVE --trust-unsigned
- or without it, with horizon-prepare.exe from a Horizon release, in this folder:
  horizon-prepare.exe --prepare --output .\\resources --server-jar <server.jar>
  horizon-prepare.exe sounds --output .\\resources
  then start the game with start.bat.

The server and every player must run Lockout $version (Fabric or Horizon).
EOF
fi
sed -i 's/$/\r/' dist/README.txt

# -h: the resources go in as the folder dist/resources links to; --hard-dereference: every file in
# full, no links for Windows to make.
rm -f "$ARCHIVE"
tar -czhf "$ARCHIVE" --hard-dereference -C dist "${files[@]}"
echo "Wrote $HORIZON/$ARCHIVE ($(du -h "$ARCHIVE" | cut -f1)). Test what went into it:"
if [ "$with_resources" = 1 ]; then
    echo "  wine dist/horizon-client.exe --resources dist/resources/"
else
    echo "  wine dist/horizon-client.exe --resources <your resources>/"
fi
