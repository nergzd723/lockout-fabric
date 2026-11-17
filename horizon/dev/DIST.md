# Builds of Horizon with Lockout for other machines

How to build Horizon with Lockout for Windows or Linux PCs on your Linux machine, without the launcher, packed with
the prepared resources so the other PCs only unpack and play (for friends, or for PCs that are offline during a LAN
event); and a dedicated Lockout server that runs without Java ([A server](#a-server)).

## Once: the setup

Two clones side by side, `~/horizon` and `~/lockout-fabric`:

```bash
git clone https://github.com/nergzd723/lockout-fabric ~/lockout-fabric
```

Put the Horizon checkout on the commit the mod pins (launcher 0.2.69's, `launcher@0.2.69`):

```bash
cd ~/horizon && git fetch origin --tags && git checkout "$(sed -n 's/^horizon_revision = "\(.*\)"/\1/p' ~/lockout-fabric/horizon/lockout/horizon-mixin.toml)"
```

The build scripts add the runner (`horizon/dev/lockout-run`) to the workspace themselves, and refresh it from the
mod on every build.

The resources, in `resources/` at the checkout's root, from your own Minecraft 26.3 (played once from the official
launcher) and Java 25; `--complete` adds the sounds:

```bash
cd ~/horizon && cargo run --release -p horizon-prepare -- --prepare --complete --output resources
```

(The first time, add `--server-jar <server.jar>`: the 26.3 server jar you downloaded.)

For Windows builds only, the Windows target and the MinGW linker:

```bash
rustup target add x86_64-pc-windows-gnu
```

```bash
sudo apt install mingw-w64
```

(On Arch: `sudo pacman -S mingw-w64-gcc`.)

## Every build

From `~/horizon`, for Windows:

```bash
bash ~/lockout-fabric/horizon/dev/build-windows.sh
```

or for Linux:

```bash
bash ~/lockout-fabric/horizon/dev/build-linux.sh
```

or for Linux on 64-bit ARM (see [Linux on ARM](#linux-on-arm-and-older-distributions) for the setup):

```bash
bash ~/lockout-fabric/horizon/dev/build-linux-arm64.sh
```

(They are `build-dist.sh --windows`, `--linux` or `--linux-arm64`.) Each checks the setup and the resources first
(on the pinned commit, the runner up to date, `horizon-prepare --check` passes), then builds and writes the archive
in `~/horizon`:

| | Archive | Holds |
| --- | --- | --- |
| Windows | `lockout-horizon-windows-x64.tar.gz` | `horizon-client.exe`, `start.bat`, `README.txt`, `resources/` |
| Linux | `lockout-horizon-linux-x64.tar.gz` | `horizon-client`, `start.sh`, `README.txt`, `resources/` |
| Linux on ARM | `lockout-horizon-linux-arm64.tar.gz` | the same, for 64-bit ARM |

A resources folder elsewhere goes as an argument: `build-linux.sh ~/elsewhere/resources`.

The checks after the build:

- The game must need only the libraries every player has: the script runs Horizon's `tools/check_imports.py` on it,
  as Horizon's own release does. For Windows, that's Windows' own DLLs: a `libwinpthread-1.dll` or
  `libgcc_s_seh-1.dll` there means the static linking didn't take (Wine finds those DLLs on your system, a Windows
  PC won't). For Linux, glibc, ALSA and udev.
- The Windows exe reserves a 64 MiB stack: Windows' default is 1 MiB and the game overflows it right after the
  loading screen. `lockout-run/build.rs` sets it (Horizon's own `horizon-client` reserves 8 MiB in its `build.rs`,
  which `lockout-run` doesn't run); the script checks it is there.
- The Linux build prints the newest glibc it needs. It is built against your system's glibc, so it runs on
  distributions with that version or newer (`ldd --version` shows a PC's). Arch's is newer than most: a build from
  Arch may not start on Ubuntu or Debian (Horizon builds its own Linux release on Ubuntu 22.04 for that). Build on
  the oldest system the players have, such as an Ubuntu 22.04 container or VM, to reach them all.

Test what went into the archive (`dist/windows/` or `dist/linux/` holds it, its `resources` linking to yours), not
an older copy:

```bash
wine dist/windows/horizon-client.exe --resources dist/windows/resources/
```

```bash
dist/linux/start.sh
```

Get past the title screen: make a world, start a short `/lockout players A B` match.

## Linux on ARM, and older distributions

On a 64-bit ARM Linux machine (a Raspberry Pi 5, an ARM server), do the setup there and run
`build-linux-arm64.sh` (or `build-linux.sh`, the same there): it builds natively.

From an x86-64 machine, the ARM build is cross-compiled, and needs ARM copies of ALSA and udev to link with. Debian
and Ubuntu install those beside their own (multiarch); Arch can't. So on Arch, build inside an Ubuntu 22.04
[distrobox](https://distrobox.it/): it shares your home folder, so `~/horizon`, `~/lockout-fabric` and your Rust
toolchains are all there. Built in it, the x64 Linux archive needs only glibc 2.35, so it also runs on Ubuntu
22.04 and later, Debian 12 and the like, which a build from Arch doesn't.

```bash
distrobox create --name lockout-build --image ubuntu:22.04
```

```bash
distrobox enter lockout-build
```

Once, inside it: arm64 packages come from Ubuntu's ports mirror, so the usual mirror is kept to amd64.

```bash
sudo dpkg --add-architecture arm64 && sudo sed -i 's/^deb http/deb [arch=amd64] http/' /etc/apt/sources.list
```

```bash
printf '%s\n' 'deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy main universe' 'deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy-updates main universe' | sudo tee /etc/apt/sources.list.d/arm64.list
```

```bash
sudo apt update && sudo apt install -y build-essential pkg-config python3 libasound2-dev libudev-dev gcc-aarch64-linux-gnu libasound2-dev:arm64 libudev-dev:arm64
```

```bash
rustup target add aarch64-unknown-linux-gnu
```

(If `rustup` or `cargo` isn't found in the box, `source ~/.cargo/env` first.) Then build there with a target
folder of its own, so its builds don't mix with the ones made on Arch:

```bash
cd ~/horizon && CARGO_TARGET_DIR=target/ubuntu-22.04 bash ~/lockout-fabric/horizon/dev/build-linux-arm64.sh
```

```bash
cd ~/horizon && CARGO_TARGET_DIR=target/ubuntu-22.04 bash ~/lockout-fabric/horizon/dev/build-linux.sh
```

On Debian or Ubuntu itself, the same steps work without the box (on Ubuntu 24.04 the mirror list is
`/etc/apt/sources.list.d/ubuntu.sources`: add `Architectures: amd64` to each entry instead of the `sed`).

Horizon makes no Linux ARM release of its own, so it isn't a platform Horizon tests: the build is checked to link
and to need only standard libraries, but whether a given ARM board's GPU driver runs the game is for that board to
show.

## A server

Horizon's dedicated server is two programs: `horizon-server`, the supervisor (one settings file, `server.toml`, a
console, a web panel, backups; Horizon's [docs/hosting.md](https://github.com/horizon-rust/horizon/blob/4626921895df51521be64d485ee763412de2917c/docs/hosting.md)),
and the engine it runs, the game without sound or controllers. `build-server.sh` builds both from the pinned
checkout, with Lockout in the engine, so a Lockout server needs no Java. Vanilla 26.3 clients can't play Lockout
on it (the mod kicks a client without Lockout, as the Fabric server does); Fabric and Horizon clients with Lockout
can.

Build it static, with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild): Zig compiles the C parts and
links musl into the programs, so they need no libc on the server at all and run on any Linux of their CPU, whatever
glibc it has; and an ARM server needs no cross compiler. (Only the server can be: the game loads the GPU's
libraries at run time, which a static program can't.) Once, on Arch:

```bash
sudo pacman -S zig
```

```bash
cargo install --locked cargo-zigbuild
```

```bash
rustup target add aarch64-unknown-linux-musl x86_64-unknown-linux-musl
```

(Elsewhere, `pip install ziglang` also gives cargo-zigbuild its Zig.) Then, for a 64-bit ARM server (`uname -m`
says `aarch64`; Oracle Cloud's Ampere machines, for one):

```bash
cd ~/horizon && bash ~/lockout-fabric/horizon/dev/build-server.sh --linux-arm64
```

or for an x86-64 one (`x86_64`):

```bash
cd ~/horizon && bash ~/lockout-fabric/horizon/dev/build-server.sh
```

It writes `lockout-horizon-server-linux-arm64.tar.gz` (or `-x64`): `horizon-server`, `horizon-engine`, a
`README.txt`, and your prepared `resources/` (the server is yours, so your copy's files are yours to put on it;
`--no-resources` leaves them out). The script checks both programs are static ("static, needs no library").

Without cargo-zigbuild, the script links against this machine's glibc instead (an ARM build then needs Debian or
Ubuntu's `gcc-aarch64-linux-gnu`, as in [Linux on ARM](#linux-on-arm-and-older-distributions)). The programs then
run only on systems with that glibc or newer, which the script prints: compare with `ldd --version` on the server.
Arch's is newer than an Ubuntu server's, so that way needs the Ubuntu 22.04 distrobox.

On the server, work in a copy of the server's folder, so the Fabric one stays as it is to go back to (the world,
`server.properties`, ops, whitelist, `config/lockout.json` and Lockout's saved match come along; the Fabric jar,
`mods/` and `libraries/` are left unused):

```bash
cp -a ~/minecraft-26.3 ~/minecraft-26.3-horizon
```

Copy the archive there from your computer:

```bash
scp lockout-horizon-server-linux-arm64.tar.gz ubuntu@<server>:minecraft-26.3-horizon/
```

then, on the server:

```bash
cd ~/minecraft-26.3-horizon && tar -xf lockout-horizon-server-linux-arm64.tar.gz
```

Once: `init` asks about the EULA and the world, imports `server.properties` into `server.toml` (keeping the original
as `server.properties.imported`), and keeps the existing world.

```bash
./horizon-server init
```

Stop the Fabric server first (both listen on 25565), then start this one with its console on the terminal (Ctrl-C
stops it):

```bash
./horizon-server run
```

or in the background, with `./horizon-server status` and `./horizon-server stop`:

```bash
./horizon-server start
```

The Lockout commands, permissions and messages are the Fabric server's, and the saved match continues where the
Fabric server left it. The server build doesn't include `lockout_tweaks` (raids on every difficulty as on Normal,
zombies always converting villagers), as none of these builds do.

For a new Lockout version, build again and unpack the new archive over the old programs; `server.toml` and the
world stay.

## Minecraft's files

`resources/` is Minecraft's own files, prepared from your copy. Mojang's assets can't be handed out freely: send the
archive only to friends who own Minecraft: Java Edition, and never post it (not in a GitHub release, not in a
public link). `--no-resources` packs one without them:

```bash
bash ~/lockout-fabric/horizon/dev/build-windows.sh --no-resources
```

Each PC then prepares its own from its own Minecraft 26.3 install (with Java 25), while it still has internet:

- with the launcher: `horizon-launcher install latest`, `horizon-launcher prepare`, then
  `horizon-launcher import-build lockout-horizon-windows-x64.tar.gz --trust-unsigned` (or the Linux archive);
- or without it, with `horizon-prepare` from a Horizon release, in the unpacked folder:
  `horizon-prepare --prepare --output resources --server-jar <server.jar>` and
  `horizon-prepare sounds --output resources`, then the start script.

## On each PC

Windows: unpack the archive anywhere, in Command Prompt or PowerShell (Windows 10 1803 and later),

```
tar -xf lockout-horizon-windows-x64.tar.gz
```

or open it in Windows 11's Explorer. Then start the game with `start.bat` (it runs
`horizon-client.exe --resources .\resources`). `dxcompiler.dll`, which Horizon's own Windows release carries,
isn't needed: without it Horizon compiles its shaders with Windows' own FXC.

Linux (x64 or ARM): unpack it anywhere and start the game with `./start.sh` (it runs `./horizon-client --resources ./resources`).
It needs a Vulkan driver, ALSA and udev, which desktop distributions have.

```bash
tar -xf lockout-horizon-linux-x64.tar.gz && ./start.sh
```

## A new Lockout version

1. Update the mod:

   ```bash
   cd ~/lockout-fabric && git pull
   ```

2. If the mod now pins another Horizon commit, the script says so. Undo the script's workspace edit (it blocks the
   checkout), then redo the setup's checkout step; the script adds the runner again:

   ```bash
   cd ~/horizon && git checkout -- Cargo.toml && rm -rf lockout-run
   ```

   The script then runs `horizon-prepare --check` on the resources and stops with what to run if the new commit
   needs more.

3. Build as above, then send everyone the new archive and have them unpack it over the old folder.

4. Everyone moves together: the server and every client must run the same Lockout version (Fabric or Horizon), or
   the server kicks the client with "Wrong Lockout version".
