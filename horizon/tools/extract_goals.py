#!/usr/bin/env python3
"""Builds horizon/lockout/api/data/goals.json from the Fabric mod's Java sources.

The Horizon port shares goal ids, names and board rules with the Java mod, so a
Fabric client and a Horizon client show the same board. Run it after a goal is
added or changed on the Java side:

    python3 horizon/tools/extract_goals.py
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
    m = re.search(r"\.biomes\(List\.of\(([^)]*)\)\)", body)
    if m:
        req["biomes"] = ["minecraft:" + s.strip().split(".")[-1].lower() for s in m.group(1).split(",") if s.strip()]
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
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps({"goals": goals, "groups": groups_out}, indent=1) + "\n")
    print(f"{len(goals)} goals, {len(groups_out)} groups -> {OUT.relative_to(ROOT)}", file=sys.stderr)


if __name__ == "__main__":
    main()
