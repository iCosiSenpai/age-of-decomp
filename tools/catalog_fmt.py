#!/usr/bin/env python3
"""Writes apps/launcher/src/lib/settings/catalog.json in its own layout (three lines per setting, one per choice).

Run it after a script changed the catalogue through `json`: `py -I tools/catalog_fmt.py`. `--check` only says whether
the file is in that layout (exit 1 when it is not). The data is never changed: the result is parsed and compared.
"""
import json
import sys
from pathlib import Path

PATH = Path(__file__).resolve().parent.parent / "apps" / "launcher" / "src" / "lib" / "settings" / "catalog.json"
HEAD = ("key", "section", "group", "kind", "source", "restart", "advanced")
SECOND = ("label", "default", "min", "max", "step", "format", "unit", "when")
KNOWN = set(HEAD) | set(SECOND) | {"help", "choices"}


def js(value):
    return json.dumps(value, ensure_ascii=False)


def pair(key, value):
    if isinstance(value, dict):
        return f"{js(key)}: {{ " + ", ".join(f"{js(a)}: {js(b)}" for a, b in value.items()) + " }"
    return f"{js(key)}: {js(value)}"


def setting(s):
    unknown = set(s) - KNOWN
    if unknown:
        raise SystemExit(f"catalog_fmt: setting {s.get('key')!r} has fields this layout does not know: {sorted(unknown)}")
    lines = ["      " + ", ".join(pair(k, s[k]) for k in HEAD if k in s) + ",",
             "      " + ", ".join(pair(k, s[k]) for k in SECOND if k in s) + ",",
             "      " + pair("help", s["help"])]
    if "choices" in s:
        lines[-1] += ","
        lines.append('      "choices": [')
        last = len(s["choices"]) - 1
        lines += [f"        {{ {pair('value', c['value'])}, {pair('label', c['label'])} }}" + ("" if i == last else ",")
                  for i, c in enumerate(s["choices"])]
        lines.append("      ]")
    return "    {\n" + "\n".join(lines) + "\n    }"


def render(cat):
    extra = set(cat) - {"$comment", "file", "groups", "settings"}
    if extra:
        raise SystemExit(f"catalog_fmt: fields this layout does not know: {sorted(extra)}")
    out = ["{", "  " + pair("$comment", cat["$comment"]) + ",", "  " + pair("file", cat["file"]) + ",", '  "groups": [']
    last = len(cat["groups"]) - 1
    out += ["    { " + ", ".join(pair(k, v) for k, v in g.items()) + " }" + ("" if i == last else ",")
            for i, g in enumerate(cat["groups"])]
    out += ["  ],", '  "settings": [', ",\n".join(setting(s) for s in cat["settings"]), "  ]", "}"]
    return "\n".join(out) + "\n"


def main():
    old = PATH.read_text(encoding="utf-8")
    cat = json.loads(old)
    new = render(cat)
    if json.loads(new) != cat:
        raise SystemExit("catalog_fmt: the layout would change the data; nothing written")
    if "--check" in sys.argv[1:]:
        if new != old:
            raise SystemExit("catalog_fmt: catalog.json is not in its layout; run py -I tools/catalog_fmt.py")
        return
    if new != old:
        PATH.write_text(new, encoding="utf-8", newline="\n")
    print(f"catalog_fmt: {len(cat['settings'])} settings, {'rewritten' if new != old else 'already in its layout'}")


if __name__ == "__main__":
    main()
