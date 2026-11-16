#!/usr/bin/env python3
"""Packs a Horizon bundle into the .hnmod the launcher makes of it.

Mirrors `pack` in Horizon launcher 0.2.69 (launcher/src/github_mods.rs): a zip of
`horizon-native.json` (schema 1, id, version, game, the pinned Horizon and horizon-mixins
commits) and the bundle's files under `mixins/<id>/`, kept only where a registry bundle may
carry them (Rust, TOML, `data/` files, READMEs and licences). Check the result with Horizon's
`cargo horizon-mod check <file>`.

    python3 -I horizon/tools/pack_hnmod.py horizon/lockout lockout-0.10.4.hnmod
"""
import json
import sys
import tomllib
import zipfile
from pathlib import Path

GAME = "26.3"
# The horizon-mixins commit launcher 0.2.69 builds with (native_mods::MIXINS_REV).
MIXINS_REV = "5a64af938ec86b00d53d16a408aabc0038fb3e57"


def in_data(rest):
    parts = rest.split("/")
    return len(parts) >= 2 and (parts[0] == "data" or (len(parts) >= 3 and parts[1] == "data"))


def text(rest):
    parts = rest.split("/")
    name = parts[-1]
    starts = ("README", "LICENSE", "LICENCE", "COPYING", "NOTICE")
    return len(parts) <= 2 and (name.startswith(starts) or name == ".gitignore")


def allowed(rest):
    return rest.endswith(".rs") or rest.endswith(".toml") or in_data(rest) or text(rest)


def pack(bundle: Path, out: Path):
    manifest_toml = tomllib.loads((bundle / "horizon-mixin.toml").read_text())
    mod = manifest_toml["mod"]
    horizon = manifest_toml.get("compatibility", {}).get("horizon_revision", "")
    if len(horizon) != 40:
        raise SystemExit(f"{mod['id']} has no [compatibility] horizon_revision (40 hex digits)")
    manifest = {
        "schema": 1,
        "id": mod["id"],
        "version": mod["version"],
        "game": GAME,
        "horizon": horizon,
        "mixins": MIXINS_REV,
    }
    files = []
    for path in sorted(bundle.rglob("*")):
        if path.is_symlink():
            raise SystemExit(f"{path} is a link: not allowed in a mod")
        if path.is_file():
            files.append(path.relative_to(bundle).as_posix())
    kept = [f for f in files if allowed(f)]
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("horizon-native.json", json.dumps(manifest, indent=2))
        for f in kept:
            info = zipfile.ZipInfo(f"mixins/{mod['id']}/{f}", date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            z.writestr(info, (bundle / f).read_bytes())
    print(f"{out}: {mod['id']} {mod['version']}, {len(kept)} of {len(files)} files, Horizon {horizon[:7]}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: pack_hnmod.py <bundle folder> <out.hnmod>")
    pack(Path(sys.argv[1]), Path(sys.argv[2]))
