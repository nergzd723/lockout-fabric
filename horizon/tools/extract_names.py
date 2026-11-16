#!/usr/bin/env python3
"""Writes horizon/lockout/api/data/names.json: the English names the Java mod's tooltips print.

The Java mod reads them from the game's language (`Item.getName`, `EntityType.getDescription`,
an advancement's display title). A Horizon server has no language table for mods, so the
names come from the Minecraft client jar of the version the mod targets:

    python3 -I horizon/tools/extract_names.py path/to/26.3/client.jar

The jar is at https://piston-meta.mojang.com/mc/game/version_manifest_v2.json -> 26.3 ->
downloads.client.
"""
import json
import pathlib
import sys
import zipfile

OUT = pathlib.Path(__file__).resolve().parent.parent / "lockout" / "api" / "data" / "names.json"


def text(component, lang):
    """The English of a text component, as `Component.getString` reads it (no arguments)."""
    if isinstance(component, str):
        return component
    if isinstance(component, list):
        return "".join(text(c, lang) for c in component)
    out = ""
    if "translate" in component:
        out = lang.get(component["translate"], component.get("fallback", component["translate"]))
    elif "text" in component:
        out = component["text"]
    for extra in component.get("extra", []):
        out += text(extra, lang)
    return out


def main(jar_path):
    jar = zipfile.ZipFile(jar_path)
    lang = json.loads(jar.read("assets/minecraft/lang/en_us.json"))
    items, entities, blocks = {}, {}, {}
    for key, value in lang.items():
        parts = key.split(".")
        if len(parts) != 3 or parts[1] != "minecraft":
            continue
        kind, _, path = parts
        if kind == "item":
            items[f"minecraft:{path}"] = value
        elif kind == "block":
            blocks[f"minecraft:{path}"] = value
        elif kind == "entity":
            entities[f"minecraft:{path}"] = value
    # A block's item is named after the block.
    for block, name in blocks.items():
        items.setdefault(block, name)
    advancements = {}
    prefix = "data/minecraft/advancement/"
    for name in jar.namelist():
        if not (name.startswith(prefix) and name.endswith(".json")):
            continue
        display = json.loads(jar.read(name)).get("display")
        if display is not None:
            advancements["minecraft:" + name[len(prefix):-5]] = text(display["title"], lang)
    out = {
        "items": dict(sorted(items.items())),
        "blocks": dict(sorted(blocks.items())),
        "entities": dict(sorted(entities.items())),
        "advancements": dict(sorted(advancements.items())),
    }
    OUT.write_text(json.dumps(out, indent=0, ensure_ascii=False) + "\n")
    print(f"{OUT}: {len(items)} items, {len(blocks)} blocks, {len(entities)} entities, {len(advancements)} advancements")


if __name__ == "__main__":
    main(sys.argv[1])
