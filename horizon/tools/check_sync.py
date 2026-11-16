#!/usr/bin/env python3
"""Fails when the Horizon port has drifted from the Fabric mod (CI, and before a release).

- goals.json matches the Java goals (`extract_goals.py --check`);
- one version everywhere: gradle.properties `mod_version`, `wire::LOCKOUT_VERSION` (both sides
  refuse another), the bundles' `[mod] version` and the crates';
- both bundles pin the same Horizon commit;
- the bundles pack as the launcher packs them (`pack_hnmod.py`).

    python3 -I horizon/tools/check_sync.py
"""
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "horizon/tools"
BUNDLES = [ROOT / "horizon/lockout", ROOT / "horizon/.lockout_tweaks"]


def main():
    problems = []
    if subprocess.run([sys.executable, "-I", str(TOOLS / "extract_goals.py"), "--check"]).returncode != 0:
        problems.append("goals.json is out of date")

    java = re.search(r"^mod_version=(.+)$", (ROOT / "gradle.properties").read_text(), re.M).group(1).strip()
    wire = re.search(r'pub const LOCKOUT_VERSION: &str = "([^"]+)"', (ROOT / "horizon/lockout/api/src/wire.rs").read_text())
    versions = {
        "gradle.properties mod_version": java,
        "wire::LOCKOUT_VERSION": wire.group(1) if wire else None,
    }
    for crate in ["lockout/api", "dev/lockout-run", "dev/lockout-test"]:
        manifest = tomllib.loads((ROOT / "horizon" / crate / "Cargo.toml").read_text())
        versions[f"{crate}/Cargo.toml version"] = manifest["package"]["version"]
    pins = {}
    for bundle in BUNDLES:
        manifest = tomllib.loads((bundle / "horizon-mixin.toml").read_text())
        versions[f"{bundle.name}/horizon-mixin.toml version"] = manifest["mod"]["version"]
        pins[bundle.name] = manifest.get("compatibility", {}).get("horizon_revision")
    for name, version in versions.items():
        if version != java:
            problems.append(f"{name} is {version}, gradle.properties says {java}")
    if len(set(pins.values())) != 1 or None in pins.values():
        problems.append(f"the bundles pin different Horizon commits: {pins}")

    with tempfile.TemporaryDirectory() as out:
        for bundle in BUNDLES:
            result = subprocess.run(
                [sys.executable, "-I", str(TOOLS / "pack_hnmod.py"), str(bundle), str(Path(out) / f"{bundle.name}.hnmod")]
            )
            if result.returncode != 0:
                problems.append(f"{bundle.name} doesn't pack")

    for p in problems:
        print(f"error: {p}", file=sys.stderr)
    if problems:
        raise SystemExit(1)
    print(f"In step: version {java}, Horizon {next(iter(pins.values()))[:7]}", file=sys.stderr)


if __name__ == "__main__":
    main()
