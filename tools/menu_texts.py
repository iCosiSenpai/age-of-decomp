"""The in-game menu's text catalogue (engine/source/runtime/ui_text.c): which ids lack a language, which are unused.

  py -I tools/menu_texts.py [--engine DIR] missing            ids of the menu range without a text, per language
  py -I tools/menu_texts.py [--engine DIR] unused             menu ids no source file names besides the catalogue
  py -I tools/menu_texts.py [--engine DIR] dump LANG          every menu id with its English and LANG text (JSON lines)

The catalogue is C: one array per language with `[UI_ID] = "text" "more",` entries, and `#if RT_TARGET_HWAOI` /
`#else` / `#endif` blocks, of which this game takes the first branch. Standard library only.
"""
import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MENU_LANGS = ("en", "ru", "it", "zh-Hans")


def strip_other_target(text):
    """Keep the RT_TARGET_HWAOI branch of every #if RT_TARGET_HWAOI ... #else ... #endif."""
    out, keep, depth = [], [True], 0
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("#if"):
            depth += 1
            if "RT_TARGET_HWAOI" in s:
                keep.append(not s.startswith("#if !") and "!RT_TARGET_HWAOI" not in s)
            else:
                keep.append(keep[-1])
            continue
        if s.startswith("#else"):
            keep[-1] = not keep[-1] if len(keep) > 1 else keep[-1]
            continue
        if s.startswith("#endif"):
            depth -= 1
            if len(keep) > 1:
                keep.pop()
            continue
        if all(keep):
            out.append(line)
    return "\n".join(out)


def c_string(parts):
    """The pieces of a C string literal joined, with the escapes the catalogue uses."""
    text = "".join(parts)
    return re.sub(r"\\(.)", lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)), text)


ENTRY = re.compile(r'\[(UI_[A-Z0-9_]+)\]\s*=\s*((?:"(?:[^"\\]|\\.)*"\s*)+)', re.S)
PIECE = re.compile(r'"((?:[^"\\]|\\.)*)"')


def tables(source):
    """{language: {id: text}} from the catalogue's arrays (static const char *const <name>[UI_TEXT_COUNT] = {...})."""
    text = strip_other_target(source)
    found = {}
    # rows[] names each array with its language: {"en", text_en}, ...
    rows = re.search(r"rows\[\]\s*=\s*\{(.*?)\};", text, re.S)
    names = dict(re.findall(r'\{\s*"([^"]+)"\s*,\s*([A-Za-z0-9_]+)\s*\}', rows.group(1))) if rows else {}
    for lang, array in names.items():
        m = re.search(r"\b" + re.escape(array) + r"\s*\[[^\]]*\]\s*=\s*\{", text)
        if not m:
            continue
        end = text.find("\n};", m.end())
        body = text[m.end():end]
        found[lang] = {i: c_string(PIECE.findall(s)) for i, s in ENTRY.findall(body)}
    return found


def menu_ids(header):
    """The ids from UI_MENU_FIRST to UI_MENU_LAST_ID, in order."""
    text = strip_other_target(header)
    body = text[text.index("UI_MENU_FIRST"):]
    last = re.search(r"UI_MENU_LAST_ID\s*=\s*(UI_[A-Z0-9_]+)", body).group(1)
    ids = []
    for token in re.findall(r"\b(UI_[A-Z0-9_]+)\b(?:\s*=\s*UI_[A-Z0-9_]+)?", re.sub(r"//[^\n]*", "", body)):
        if token in ("UI_MENU_FIRST", "UI_MENU_LAST_ID") or token in ids:
            continue
        ids.append(token)
        if token == last:
            break
    return ids


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--engine", type=Path, default=ROOT / "engine" / "source")
    parser.add_argument("command", choices=("missing", "unused", "dump"))
    parser.add_argument("lang", nargs="?")
    args = parser.parse_args(argv)
    runtime = args.engine / "runtime"
    table = tables((runtime / "ui_text.c").read_text(encoding="utf-8"))
    ids = menu_ids((runtime / "ui_text.h").read_text(encoding="utf-8"))
    # Ids without an English entry are not catalogue text (the version, each language's own name): left out.
    ids = [i for i in ids if table["en"].get(i)]
    if args.command == "missing":
        bad = 0
        for lang in MENU_LANGS:
            lacking = [i for i in ids if not table.get(lang, {}).get(i)]
            bad += len(lacking)
            print(f"{lang}: {len(ids) - len(lacking)} of {len(ids)}" + (f", missing {len(lacking)}" if lacking else ""))
            for i in lacking:
                print(f"  {i} = {json.dumps(table['en'].get(i, ''), ensure_ascii=False)}")
        return 1 if bad else 0
    if args.command == "unused":
        sources = [p for pattern in ("*.c", "*.cpp", "*.h", "ui/*.cpp", "ui/*.h", "platform/*.c")
                   for p in runtime.glob(pattern) if p.name != "ui_text.c"]
        sources += list((args.engine / "tests").glob("*.cpp"))
        used = set()
        for p in sources:
            body = p.read_text(encoding="utf-8", errors="replace")
            if p.name == "ui_text.h":   # the enum itself names every id: only the static asserts and ranges count
                body = body[body.find("UI_TEXT_COUNT"):]
            used.update(re.findall(r"\bUI_[A-Z0-9_]+\b", body))
        for i in ids:
            if i not in used:
                print(i, "=", json.dumps(table["en"].get(i, ""), ensure_ascii=False))
        return 0
    lang = args.lang or "it"
    for i in ids:
        print(json.dumps({"id": i, "en": table["en"].get(i, ""), lang: table.get(lang, {}).get(i, "")}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
