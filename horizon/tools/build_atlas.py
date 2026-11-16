#!/usr/bin/env python3
"""Packs the Fabric mod's textures into one atlas the Horizon client uploads.

Horizon gives a mod one RGBA image (at most 1024x1024) to draw sprites from. Goal textures
(`textures/custom/**`, 80x80 and up, drawn at 16x16) go in 64x64 cells; the board's
nine-slice frames (`textures/gui/sprites/*`) keep their own size. Writes
`horizon/lockout/api/data/atlas.png` and `atlas.json` (each texture's `[x, y, w, h]`).

    python3 horizon/tools/build_atlas.py
"""
import json
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
TEXTURES = ROOT / "src/main/resources/assets/lockout/textures"
OUT = ROOT / "horizon/lockout/api/data"
CELL = 64
SIZE = 1024


def main():
    custom = sorted(p for p in (TEXTURES / "custom").rglob("*.png"))
    sprites = sorted(p for p in (TEXTURES / "gui/sprites").glob("*.png"))
    per_row = SIZE // CELL
    rows = (len(custom) + per_row - 1) // per_row
    sprite_row = rows * CELL
    height = sprite_row + max(Image.open(p).height for p in sprites)
    atlas = Image.new("RGBA", (SIZE, height), (0, 0, 0, 0))
    index = {}
    for i, path in enumerate(custom):
        x, y = (i % per_row) * CELL, (i // per_row) * CELL
        image = Image.open(path).convert("RGBA").resize((CELL, CELL), Image.LANCZOS)
        atlas.paste(image, (x, y))
        index[path.relative_to(TEXTURES).with_suffix("").as_posix()] = [x, y, CELL, CELL]
    x = 0
    for path in sprites:
        image = Image.open(path).convert("RGBA")
        atlas.paste(image, (x, sprite_row))
        index[path.relative_to(TEXTURES).with_suffix("").as_posix()] = [x, sprite_row, image.width, image.height]
        x += image.width
    if height > SIZE:
        raise SystemExit("the atlas doesn't fit Horizon's 1024x1024 mod image")
    OUT.mkdir(parents=True, exist_ok=True)
    atlas.save(OUT / "atlas.png", optimize=True)
    (OUT / "atlas.json").write_text(json.dumps({"size": [SIZE, height], "sprites": index}, indent=1) + "\n")
    print(f"{len(index)} textures, {SIZE}x{height} -> {(OUT / 'atlas.png').relative_to(ROOT)}")


if __name__ == "__main__":
    main()
