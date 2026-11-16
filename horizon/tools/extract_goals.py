#!/usr/bin/env python3
"""Builds horizon/lockout/api/data/goals.json from the Fabric mod's Java sources.

The Horizon port shares goal ids, names and board rules with the Java mod, so a
Fabric client and a Horizon client show the same board. Run it after a goal is
added or changed on the Java side:

    python3 horizon/tools/extract_goals.py

With --check it writes nothing and fails when goals.json is out of date (CI).
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JAVA = ROOT / "src/main/java/me/marin/lockout"
OUT = ROOT / "horizon/lockout/api/data/goals.json"


def call_args(text, start):
    """The text between the parenthesis at `start` and its match."""
    depth = 0
    for i in range(start, len(text)):
        if text[i] == "(":
            depth += 1
        elif text[i] == ")":
            depth -= 1
            if depth == 0:
                return text[start + 1 : i]
    raise ValueError("unbalanced")


def ids(pattern, text):
    return ["minecraft:" + m.lower() for m in re.findall(pattern, text)]


def uniq(xs):
    seen, out = set(), []
    for x in xs:
        if x not in seen:
            seen.add(x)
            out.append(x)
    return out


def goal_types():
    src = (JAVA / "lockout/GoalType.java").read_text()
    return dict(re.findall(r'String (\w+)\s*=\s*"([^"]+)";', src))


def requirements():
    """GoalRequirements' named constants: structures, biomes, team size rules."""
    src = (JAVA / "generator/GoalRequirements.java").read_text()
    out = {}
    for name, body in re.findall(r"public static final GoalRequirements (\w+) = new Builder\(\)(.*?)\.build\(\);", src, re.S):
        out[name] = parse_requirement_body(body)
    return out


def parse_requirement_body(body):
    req = {}
    m = re.search(r"\.structures\(List\.of\(([^)]*)\)\)", body)
    if m:
        req["structures"] = ["minecraft:" + s.strip().split(".")[-1].lower() for s in m.group(1).split(",") if s.strip()]
    m = re.search(r"\.(biomes|allBiomes)\(List\.of\(([^)]*)\)\)", body)
    if m:
        req["biomes"] = ["minecraft:" + s.strip().split(".")[-1].lower() for s in m.group(2).split(",") if s.strip()]
        if m.group(1) == "allBiomes":
            req["all_biomes"] = True
    m = re.search(r"\.oneBiomeFromEach\(List\.of\((.*)\)\)", body, re.S)
    if m:
        groups = re.findall(r"List\.of\(([^)]*)\)", m.group(1))
        req["biome_groups"] = [["minecraft:" + s.strip().split(".")[-1].lower() for s in g.split(",") if s.strip()] for g in groups]
    if "partOfRandomPool(false)" in body:
        req["random_pool"] = False
    m = re.search(r"isTeamSizeOk\(\(size\) -> size ([>=]=) (\d+)\)", body)
    if m:
        req["teams_min" if m.group(1) == ">=" else "teams_exactly"] = int(m.group(2))
    return req


def registrations(named_reqs):
    src = (JAVA / "lockout/DefaultGoalRegister.java").read_text()
    out = []
    for m in re.finditer(r"INSTANCE\.register\(", src):
        args = call_args(src, m.end() - 1)
        gm = re.match(r"\s*GoalType\.(\w+),\s*(\w+)\.class\s*(?:,(.*))?$", args, re.S)
        if not gm:
            raise SystemExit(f"can't read registration: {args[:80]}")
        const, cls, rest = gm.group(1), gm.group(2), gm.group(3) or ""
        req = {}
        nm = re.match(r"\s*GoalRequirements\.(\w+)", rest)
        if nm:
            req = dict(named_reqs[nm.group(1)])
        elif "new GoalRequirements.Builder()" in rest:
            req = parse_requirement_body(rest.split(".build()")[0])
        data = None
        if "withLeatherArmorPiece" in rest:
            data = "leather_dye"
        elif "withDye" in rest:
            data = "dye"
        out.append((const, cls, req, data))
    return out


def class_files():
    files = {}
    for p in (JAVA / "lockout/goals").rglob("*.java"):
        files[p.stem] = p
    return files


STRING = r'"((?:[^"\\]|\\.)*)"'


def name_template(expr):
    """A Java string expression as a template: `{amount}` for getAmount(), `{color}` for a dye."""
    expr = expr.strip()
    fm = re.match(r"String\.format\(" + STRING + r"\s*,\s*getAmount\(\)\s*\)$", expr)
    if fm:
        return re.sub(r"%d", "{amount}", fm.group(1)).replace('\\"', '"')
    out = []
    for part in re.split(r"\s*\+\s*(?=(?:[^\"]*\"[^\"]*\")*[^\"]*$)", expr):
        sm = re.fullmatch(STRING, part)
        if sm:
            out.append(sm.group(1).replace('\\"', '"'))
        elif part == "getAmount()":
            out.append("{amount}")
        elif "getDyeColorFormatted" in part:
            out.append("{color}")
        else:
            return None
    return "".join(out)


def goal_name(text):
    m = re.search(r"getGoalName\(\)\s*\{\s*return\s+(.*?);", text, re.S)
    if not m:
        return None
    expr = m.group(1)
    if re.fullmatch(r"\w+", expr.strip()):
        am = re.search(expr.strip() + r"\s*=\s*(.*?);", text, re.S)
        if not am:
            return None
        expr = am.group(1)
    return name_template(expr)


def parse_interface(name):
    """Defaults a goal's abstract parent gives it: its name, icon and texture."""
    path = JAVA / "lockout/interfaces" / f"{name}.java"
    if not path.exists():
        return {}
    text = path.read_text()
    info = {}
    n = goal_name(text)
    if n:
        info["name"] = n
    m = re.search(r'"textures/custom/([^"]+)\.png"', text)
    if m:
        info["texture"] = m.group(1)
    m = re.search(r"(?:DISPLAY_)?ITEM(?:_STACK)?\s*=\s*(?:new ItemStack\()?Items\.([A-Z0-9_]+)", text)
    if m:
        info["icon"] = "minecraft:" + m.group(1).lower()
    return info


def parse_class(path, files):
    text = path.read_text()
    ext = re.search(r"class \w+(?:<[^>]*>)? extends (\w+)", text)
    parent = ext.group(1) if ext else "Goal"
    info = {"class": path.stem, "kind": parent, "category": path.parent.name}
    # A goal that extends another goal class (KillSnowGolemInNetherGoal) inherits its data.
    if parent in files:
        base = parse_class(files[parent], files)
        info = {**base, "class": path.stem, "category": path.parent.name}
    name = goal_name(text)
    if name:
        info["name"] = name
    m = re.search(r'"textures/custom/([^"]+)\.png"', text)
    if m:
        info["texture"] = m.group(1)
    for key, pat in [
        ("items", r"Items\.([A-Z0-9_]+)"),
        ("entities", r"EntityTypes\.([A-Z0-9_]+)"),
        ("effects", r"MobEffects\.([A-Z0-9_]+)"),
        ("damage_types", r"DamageTypes\.([A-Z0-9_]+)"),
        ("stats", r"Stats\.([A-Z0-9_]+)"),
        ("potions", r"Potions\.([A-Z0-9_]+)"),
        ("biomes", r"Biomes\.([A-Z0-9_]+)"),
    ]:
        found = uniq(ids(pat, text))
        if found:
            info[key] = found
    named = uniq(re.findall(r'fromNamespaceAndPath\("minecraft",\s*"([^"]+)"\)', text))
    if named:
        if parent in ("AdvancementGoal", "GetUniqueAdvancementsGoal"):
            info["advancements"] = ["minecraft:" + n for n in named]
        elif parent in ("VisitBiomeGoal", "VisitUniqueBiomesGoal"):
            info["biomes"] = ["minecraft:" + n for n in named]
        elif parent == "IncrementStatGoal":
            info["stats"] = ["minecraft:" + n for n in named]
        else:
            info["named"] = ["minecraft:" + n for n in named]
    m = re.search(r"getAmount\(\)\s*\{\s*return\s+(\d+)\s*;", text)
    if m:
        info["amount"] = int(m.group(1))
    m = re.search(r"Level\.(NETHER|END|OVERWORLD)\b", text)
    if m:
        info["dimension"] = {"NETHER": "minecraft:the_nether", "END": "minecraft:the_end", "OVERWORLD": "minecraft:overworld"}[m.group(1)]
    # The icon: the stack the board draws when it has no texture of its own.
    m = re.search(r"getTextureItemStack\(\)\s*\{(.*?)\n    \}", text, re.S)
    icon = None
    if m:
        body = m.group(1)
        im = re.search(r"Items\.([A-Z0-9_]+)", body)
        if im:
            icon = "minecraft:" + im.group(1).lower()
        else:
            ref = re.search(r"return\s+(\w+)", body)
            if ref and ref.group(1) != "null":
                cm = re.search(ref.group(1) + r"\s*=\s*(?:new ItemStack\()?Items\.([A-Z0-9_]+)", text)
                if cm:
                    icon = "minecraft:" + cm.group(1).lower()
    if icon:
        info["icon"] = icon
    for key, value in parse_interface(parent).items():
        info.setdefault(key, value)
    return info


# ---- how the board draws each goal -------------------------------------------------------
#
# Each goal gets `render`: layers drawn in order in its 16x16 square, as its Java class's
# `renderTexture` (or `getTextureItemStack`) draws them:
#   {"item": id}                         an item (`potion` also names its potion)
#   {"cycle_items": [ids]}               one item after another, 60 client ticks each
#   {"texture": path}                    a texture of the mod's (`custom/...`, `{data}` filled in)
#   {"cycle_textures": [paths]}          one texture after another, 60 ticks each
#   {"count": text}                      a stack count in the corner (`{amount}` filled in)
#   {"dyed_cycle": [ids]}                leather pieces in random colours, changing every 60 ticks
#   {"glint_cycle": [[ids], ...]}        enchanted armour, a random piece of each set in turn

def body_of(text, method):
    m = re.search(r"public \S+ " + method + r"\([^)]*\)\s*\{", text)
    if not m:
        return None
    depth, i = 1, m.end()
    while depth and i < len(text):
        depth += {"{": 1, "}": -1}.get(text[i], 0)
        i += 1
    return text[m.end() : i - 1]


def texture_path(expr):
    """`Identifier.fromNamespaceAndPath(Constants.NAMESPACE, "textures/custom/x.png")` -> `custom/x`."""
    m = re.search(r'"textures/((?:[^"\\]|\\.)*?)"\s*(?:\+\s*(\w+)\s*\+\s*"((?:[^"\\]|\\.)*?)")?', expr)
    if not m:
        return None
    path = m.group(1) + ("{data}" + m.group(3) if m.group(2) else "")
    return path[:-4] if path.endswith(".png") else path


def constant(text, name):
    m = re.search(r"\b" + re.escape(name) + r"\s*=\s*(.*?);", text, re.S)
    return m.group(1) if m else None


def stack_item(text, expr):
    """The item a stack expression or constant stands for."""
    expr = expr.strip()
    if re.fullmatch(r"\w+", expr):
        value = constant(text, expr)
        if value is None:
            return None
        expr = value
    m = re.search(r"Items\.([A-Z0-9_]+)", expr)
    return "minecraft:" + m.group(1).lower() if m else None


def stack_count(text, name):
    """`STACK.setCount(getAmount())` or `setCount(16)` in the constructor."""
    m = re.search(re.escape(name) + r"\.setCount\(([^;]*)\);", text)
    if not m:
        return None
    arg = m.group(1).strip()
    return "{amount}" if "getAmount" in arg else arg


def list_constant(text, pattern_item):
    out = []
    for name, body in re.findall(r"(\w+)\s*=\s*List\.of\((.*?)\);", text, re.S):
        yield name, body


def items_of(expr):
    return ["minecraft:" + i.lower() for i in re.findall(r"Items\.([A-Z0-9_]+)", expr)]


def textures_of(expr):
    return [texture_path(m) for m in re.findall(r'(fromNamespaceAndPath\([^)]*\))', expr) if texture_path(m)]


def interfaces_of(text):
    m = re.search(r"class \w+(?:<[^>]*>)?(?: extends \w+(?:<[^>]*>)?)?\s+implements ([^{]+)\{", text)
    return [i.strip().split("<")[0] for i in m.group(1).split(",")] if m else []


def class_text(name, files):
    if name in files:
        return files[name].read_text()
    path = JAVA / "lockout/interfaces" / f"{name}.java"
    return path.read_text() if path.exists() else None


def parent_of(text):
    m = re.search(r"class \w+(?:<[^>]*>)? extends (\w+)", text)
    return m.group(1) if m else None


def lookup(name, files, chain_texts):
    """A constant or method body found in the class or its parents, most specific first."""
    for t in chain_texts:
        v = constant(t, name)
        if v is not None:
            return v
    return None


def base_layers(kind, chain, files):
    """What an interface's default renderTexture draws."""
    if kind == "TextureProvider":
        for t in chain:
            body = body_of(t, "getTextureIdentifier")
            if body:
                ret = re.search(r"return\s+(.*?);", body, re.S).group(1).strip()
                path = texture_path(ret)
                if path is None and re.fullmatch(r"\w+", ret):
                    expr = lookup(ret, files, chain)
                    path = texture_path(expr) if expr else None
                    if path is None:
                        # Built in the constructor from the goal's data.
                        for t2 in chain:
                            m = re.search(re.escape(ret) + r"\s*=\s*(Identifier\.[^;]*);", t2)
                            if m:
                                path = texture_path(m.group(1))
                                break
                if path:
                    return [{"texture": path}]
        return []
    if kind == "CycleTexturesProvider":
        for t in chain:
            body = body_of(t, "getTexturesToDisplay")
            if body:
                ret = re.search(r"return\s+(.*?);", body, re.S).group(1).strip()
                expr = lookup(ret, files, chain) if re.fullmatch(r"\w+", ret) else ret
                return [{"cycle_textures": textures_of(expr or "")}]
        return []
    if kind == "CycleItemTexturesProvider":
        for t in chain:
            for method in ("getItemsToDisplay", "getItems"):
                body = body_of(t, method)
                if body and "return" in body:
                    ret = re.search(r"return\s+(.*?);", body, re.S).group(1).strip()
                    if ret == "getItems()":
                        continue
                    expr = lookup(ret, files, chain) if re.fullmatch(r"\w+", ret) else ret
                    items = items_of(expr or "")
                    if items:
                        return [{"cycle_items": items}]
        return []
    return []


def render_layers(cls, files, start=0):
    chain, names, name = [], [], cls
    while name:
        t = class_text(name, files)
        if t is None:
            break
        chain.append(t)
        names.append(name)
        name = parent_of(t)
    interfaces = []
    for t in chain:
        interfaces += interfaces_of(t)
    # The most specific renderTexture.
    for i, t in enumerate(chain):
        if i < start:
            continue
        body = body_of(t, "renderTexture")
        if body is None:
            continue
        layers = []
        text = "\n".join(chain)
        for stmt in re.split(r";\s*\n", body):
            stmt = stmt.strip()
            if not stmt:
                continue
            if "return false" in stmt:
                return None
            sm = re.search(r"(?:(\w+)\.)?super\.renderTexture", stmt)
            if sm:
                if sm.group(1):
                    layers += base_layers(sm.group(1), chain, files)
                else:
                    layers += render_layers(cls, files, i + 1) or []
                continue
            if "DYED_COLOR" in stmt:
                for t2 in chain:
                    b = body_of(t2, "getItems") or ""
                    ret = re.search(r"return\s+(\w+)", b)
                    expr = lookup(ret.group(1), files, chain) if ret else None
                    if expr:
                        return [{"dyed_cycle": items_of(expr)}]
            if "ENCHANTMENT_GLINT_OVERRIDE" in stmt:
                expr = lookup("ITEMS", files, chain) or ""
                inner = re.search(r"List\.of\((.*)\)", expr, re.S)
                names = [n.strip() for n in inner.group(1).split(",")] if inner else []
                sets = [items_of(lookup(n, files, chain) or "") for n in names]
                return [{"glint_cycle": [x for x in sets if x]}]
            im = re.search(r"context\.item\((.*?),\s*x,\s*y\)", stmt)
            if im:
                arg = im.group(1)
                cm = re.search(r"(\w+)\.get\(mod / 60\)", arg)
                if cm:
                    layers.append({"cycle_items": items_of(lookup(cm.group(1), files, chain) or "")})
                elif "getItemsToDisplay" in arg:
                    layers += base_layers("CycleItemTexturesProvider", chain, files)
                else:
                    item = stack_item(text, arg)
                    if item:
                        layers.append({"item": item})
                continue
            bm = re.search(r"context\.blit\([^,]*,\s*(\w+)", stmt)
            if bm:
                expr = lookup(bm.group(1), files, chain)
                path = texture_path(expr) if expr else None
                if path:
                    layers.append({"texture": path})
                continue
            dm = re.search(r"drawStackCount\([^,]*,[^,]*,[^,]*,\s*String\.valueOf\((.*?)\)\)", stmt)
            if dm:
                arg = dm.group(1).strip()
                layers.append({"count": "{amount}" if "getAmount" in arg else (lookup(arg, files, chain) or arg)})
                continue
            dd = re.search(r"itemDecorations\([^,]*,\s*(\w+),\s*x,\s*y(?:,\s*(.*?))?\)\s*$", stmt)
            if dd:
                if dd.group(2):
                    arg = dd.group(2).strip()
                    if arg.startswith('"'):
                        layers.append({"count": arg.strip('"')})
                    elif "getAmount" in arg:
                        layers.append({"count": "{amount}"})
                else:
                    count = stack_count(text, dd.group(1))
                    if count:
                        layers.append({"count": count})
                continue
        return layers
    for kind in ("TextureProvider", "CycleTexturesProvider", "CycleItemTexturesProvider"):
        if kind in interfaces:
            layers = base_layers(kind, chain, files)
            if layers:
                return layers
    return None


def has_tooltip(cls, files):
    """Whether the goal implements `HasTooltipInfo` (its board tooltip shows progress)."""
    name = cls
    while name:
        t = class_text(name, files)
        if t is None:
            return False
        if "HasTooltipInfo" in "".join(interfaces_of(t)):
            return True
        name = parent_of(t)
    return False


def icon_count(cls, files):
    """The count `getTextureItemStack`'s stack shows (`STACK.setCount(getAmount())`)."""
    chain, name = [], cls
    while name:
        t = class_text(name, files)
        if t is None:
            break
        chain.append(t)
        name = parent_of(t)
    for t in chain:
        body = body_of(t, "getTextureItemStack")
        if body:
            ret = re.search(r"return\s+(\w+)\s*;", body)
            if not ret:
                return None
            text = "\n".join(chain)
            return stack_count(text, ret.group(1))
    return None


def groups():
    src = (JAVA / "generator/GoalGroup.java").read_text()
    out = []
    for name, body, limit in re.findall(r"public static final GoalGroup (\w+) = new GoalGroup\(List\.of\((.*?)\),\s*(\d+)\);", src, re.S):
        out.append({"name": name, "goals": [g.strip() for g in body.split(",") if g.strip()], "limit": int(limit)})
    pred = {}
    for name, body in re.findall(r"(\w+)\.requirePredecessor\.addAll\(List\.of\((.*?)\)\);", src, re.S):
        pred[name] = [g.strip() for g in body.split(",") if g.strip()]
    for name, goal in re.findall(r"(\w+)\.requirePredecessor\.add\((\w+)\);", src):
        pred.setdefault(name, []).append(goal)
    for g in out:
        if g["name"] in pred:
            g["require_predecessor"] = pred[g["name"]]
    registered = re.findall(r"GOAL_GROUPS\.add\((\w+)\);", src)
    return [g for g in out if g["name"] in registered]


def main():
    types = goal_types()
    files = class_files()
    goals = []
    for const, cls, req, data in registrations(requirements()):
        if cls not in files:
            raise SystemExit(f"no source for {cls}")
        info = parse_class(files[cls], files)
        info["id"] = types[const]
        if has_tooltip(cls, files):
            info["tooltip"] = True
        layers = render_layers(cls, files)
        if layers:
            for layer in layers:
                # `getItems()` built from a stack (`List.of(ITEM_STACK.getItem())`): the goal's items.
                if layer.get("cycle_items") == []:
                    layer["cycle_items"] = info.get("items", [])
            if [list(l)[0] for l in layers] == ["count"]:
                layers.insert(0, {"cycle_items": info.get("items", [])})
            info["render"] = layers
        else:
            count = icon_count(cls, files)
            if count:
                info["icon_count"] = count
        if req:
            info["requirements"] = req
        if data:
            info["data"] = data
        goals.append(info)
    groups_out = groups()
    for g in groups_out:
        g["goals"] = [types[x] for x in g["goals"]]
        if "require_predecessor" in g:
            g["require_predecessor"] = [types[x] for x in g["require_predecessor"]]
    text = json.dumps({"goals": goals, "groups": groups_out}, indent=1) + "\n"
    if "--check" in sys.argv[1:]:
        if not OUT.is_file() or OUT.read_text() != text:
            raise SystemExit(
                f"{OUT.relative_to(ROOT)} is out of date with the Java goals: run python3 horizon/tools/extract_goals.py"
            )
        print(f"{len(goals)} goals, {len(groups_out)} groups: {OUT.relative_to(ROOT)} is up to date", file=sys.stderr)
        return
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text)
    print(f"{len(goals)} goals, {len(groups_out)} groups -> {OUT.relative_to(ROOT)}", file=sys.stderr)


if __name__ == "__main__":
    main()
