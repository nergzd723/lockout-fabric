# Windows builds of Horizon with Lockout

How to build a Windows `horizon-client.exe` with Lockout on Linux, without the launcher, packed with the prepared
resources so the other PCs only unpack and play (for friends, or for PCs that are offline during a LAN event).

## Once: the setup

Two clones side by side, `~/horizon` and `~/lockout-fabric`:

```bash
git clone https://github.com/nergzd723/lockout-fabric ~/lockout-fabric
```

Put the Horizon checkout on the commit the mod pins (launcher 0.2.69's, `launcher@0.2.69`):

```bash
cd ~/horizon && git fetch origin --tags && git checkout "$(sed -n 's/^horizon_revision = "\(.*\)"/\1/p' ~/lockout-fabric/horizon/lockout/horizon-mixin.toml)"
```

The build script adds the runner (`horizon/dev/lockout-run`) to the workspace itself, and refreshes it from the mod
on every build.

The Windows target and the MinGW linker:

```bash
rustup target add x86_64-pc-windows-gnu
```

```bash
sudo apt install mingw-w64
```

(On Arch: `sudo pacman -S mingw-w64-gcc`.)

The resources, in `resources/` at the checkout's root, from your own Minecraft 26.3 (played once from the official
launcher) and Java 25; `--complete` adds the sounds:

```bash
cd ~/horizon && cargo run --release -p horizon-prepare -- --prepare --complete --output resources
```

(The first time, add `--server-jar <server.jar>`: the 26.3 server jar you downloaded.)

## Every build

From `~/horizon`:

```bash
bash ~/lockout-fabric/horizon/dev/build-windows.sh
```

It checks the setup and the resources first (on the pinned commit, the runner up to date, `horizon-prepare --check`
passes), then builds and writes `lockout-horizon-windows-x64.tar.gz`: `horizon-client.exe`, `start.bat`, a
`README.txt` and `resources/`. A resources folder elsewhere goes as an argument:
`build-windows.sh ~/elsewhere/resources`.

- The exe reserves a 64 MiB stack: Windows' default is 1 MiB and the game overflows it right after the loading
  screen. `lockout-run/build.rs` sets it (Horizon's own `horizon-client` reserves 8 MiB in its `build.rs`, which
  `lockout-run` doesn't run); the script checks it is there.
- It must need only Windows' own DLLs: the script runs Horizon's `tools/check_imports.py` on it. A
  `libwinpthread-1.dll` or `libgcc_s_seh-1.dll` there means the static linking didn't take: Wine finds those DLLs
  on your system, a Windows PC won't.
- Test what went into the archive (`dist/` holds it, `dist/resources` links to your resources), not an older copy:

```bash
wine dist/horizon-client.exe --resources dist/resources/
```

Get past the title screen: make a world, start a short `/lockout players A B` match.

## Minecraft's files

`resources/` is Minecraft's own files, prepared from your copy. Mojang's assets can't be handed out freely: send the
archive only to friends who own Minecraft: Java Edition, and never post it (not in a GitHub release, not in a
public link). `--no-resources` packs one without them:

```bash
bash ~/lockout-fabric/horizon/dev/build-windows.sh --no-resources
```

Each PC then prepares its own from its own Minecraft 26.3 install (with Java 25), while it still has internet:

- with the launcher: `horizon-launcher install latest`, `horizon-launcher prepare`, then
  `horizon-launcher import-build lockout-horizon-windows-x64.tar.gz --trust-unsigned`;
- or without it: `horizon-prepare.exe --prepare --output .\resources --server-jar <server.jar>` and
  `horizon-prepare.exe sounds --output .\resources` (from a Horizon release), in the unpacked folder, then
  `start.bat`.

## On each Windows PC

Unpack the archive anywhere: in Command Prompt or PowerShell (Windows 10 1803 and later),

```
tar -xf lockout-horizon-windows-x64.tar.gz
```

or open it in Windows 11's Explorer. Then start the game with `start.bat` (it runs
`horizon-client.exe --resources .\resources`).

`dxcompiler.dll`, which Horizon's own Windows release carries, isn't needed: without it Horizon compiles its
shaders with Windows' own FXC.

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
