#!/usr/bin/env python3
"""Age of Decomp release tooling (Python 3.12+, standard library only).

  py tools/release.py manifest            write packaging/recomplauncher.json from the settings catalogue
  py tools/release.py docs                write docs/SETTINGS.md from the settings catalogue
  py tools/release.py windows [--zip]     stage out/release/AgeOfDecomp-v<ver>-Windows-x64/ (+ .zip, SHA256SUMS.txt)
  py tools/release.py check <folder>      refuse any game material in a folder (also run by `windows`)

The release holds the launcher, aod-cli, the engine bundle (out/engine/windows: toolchain + engine sources + shader
translator) and recomplauncher.json. It never holds anything from the game: the game is built on the player's PC.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "apps/launcher/src/lib/settings/catalog.json"
TAURI = ROOT / "apps/launcher/src-tauri"
MANIFEST = ROOT / "packaging/recomplauncher.json"
BUILD_ID = "659cb288b5c5b2b49853add616c0d5bf4ea4e1ba"  # update NCM 196608 exefs/main ModuleId (ADR-005)
GAME_DIR = "{install}/game"
INI = GAME_DIR + "/work/build/hwaoi196608/hwaoi.ini"
SAVES = GAME_DIR + "/work/saves/hwaoi196608"
# Extensions that must never be in a release (game content, keys, dumps).
FORBIDDEN = {".nup", ".nsp", ".nsz", ".xci", ".xcz", ".nca", ".ncz", ".nso", ".nro", ".npdm", ".keys", ".tik", ".bfres",
             ".sarc", ".zs", ".bntx", ".bfsha"}


def version() -> str:
    return json.loads((TAURI / "tauri.conf.json").read_text(encoding="utf-8"))["version"]


def manifest() -> dict:
    cat = json.loads(CATALOG.read_text(encoding="utf-8"))
    group_label = {g["id"]: g["label"] for g in cat["groups"]}
    settings = []
    for s in cat["settings"]:
        if s["kind"] == "dynamic":
            continue  # graphics card / controller pickers need live data: the Age of Decomp window has them
        row = {"id": s["key"], "label": s["label"], "help": s["help"], "group": group_label[s["group"]]}
        if s.get("advanced"):
            row["advanced"] = True
        if s["kind"] == "toggle":
            row["choices"] = [{"label": "Off", "set": {s["key"]: "0"}}, {"label": "On", "set": {s["key"]: "1"}}]
        elif s["kind"] == "choice":
            row["choices"] = [{"label": c["label"], "set": {s["key"]: c["value"]}} for c in s["choices"]]
        elif s["kind"] == "slider":
            row["slider"] = {"key": s["key"], "min": s["min"], "max": s["max"], "step": s["step"],
                             "format": s.get("format", "%g")}
        row["defaults"] = {s["key"]: s["default"]}
        if s.get("when"):
            row["when"] = s["when"]
            for k in s["when"]:
                dep = next(x for x in cat["settings"] if x["key"] == k)
                row["defaults"][k] = dep["default"]
        settings.append(row)
    return {
        "$schema": "https://recomplauncher.com/schema/recomplauncher.json",
        "format": 1,
        "name": "Age of Decomp",
        "repo": "iCosiSenpai/age-of-decomp",
        "game": {
            "kind": "iso_or_folder",
            "label": "Your Hyrule Warriors: Age of Imprisonment (Nintendo Switch 2) dump as one .nup file with the base "
                     "game and update 3.0.0 (NCM 196608).",
            "check": {"nup_build_id": BUILD_ID},
        },
        "inputs": [
            {"id": "dlc", "label": "Your DLC dump (.nup), if you have it", "role": "dlc", "optional": True,
             "formats": [{"type": "file", "extensions": ["nup"]}]},
        ],
        "prepare": {
            "program": ["aod-cli.exe", "AgeOfDecomp-x86_64.AppImage"],
            "args": ["cli", "prepare", "--game", "{game}", "--out", GAME_DIR, "--dlc={input:dlc}"],
            "label": "Building the game for this PC from your copy (about 10 minutes on a fast PC)",
            "timeout": 14400,
            "version": "1",
            "done": [GAME_DIR + "/aod-ready.json"],
        },
        "start": {
            "program": ["AgeOfDecomp.exe", "AgeOfDecomp-x86_64.AppImage"],
            "args": ["--play", "--game-dir", GAME_DIR],
        },
        "config": [{"id": "main", "format": "ini", "file": [{"path": INI}]}],
        "settings": settings,
        "groups": [{"id": "Performance", "advanced": True}, {"id": "Diagnostics", "advanced": True}],
        "saves": [SAVES],
        "folders": {"saves": [{"path": SAVES}], "logs": [{"path": GAME_DIR + "/logs"}]},
        "actions": [
            {"id": "saves", "label": "Saves", "help": "Your save data.", "open": "saves"},
            {"id": "logs", "label": "Logs", "help": "The game's logs, for bug reports.", "open": "logs"},
        ],
        "exit_codes": {
            "0": "ok",
            "64": "The game is not built yet, or the build is incomplete. Press Set Up Again.",
            "65": "Your .nup file has moved or changed since the build. Put it back where it was, or set up again.",
        },
        "looks": {"allowed": False,
                  "reason": "Age of Decomp has its own picture settings and AMD FSR; ReShade's Vulkan layer is not tested with its renderer yet."},
    }


def cmd_docs(_a) -> None:
    """docs/SETTINGS.md from the catalogue."""
    cat = json.loads(CATALOG.read_text(encoding="utf-8"))
    out = ["# Settings", "",
           "Generated by `py tools/release.py docs` from `apps/launcher/src/lib/settings/catalog.json`. The game reads them",
           "from `hwaoi.ini` next to `hwaoi.exe` (`<game folder>/work/build/hwaoi196608/`); an environment variable",
           "`HWAOI_<KEY>` overrides the file (and locks that row of the in-game menu). *Restart*: read at start only.", ""]
    for g in cat["groups"]:
        out += [f"## {g['label']}" + (" (advanced)" if g.get("advanced") else ""), "",
                "| Key | Setting | Values | Default | Restart |", "|---|---|---|---|---|"]
        for s in (x for x in cat["settings"] if x["group"] == g["id"]):
            if s["kind"] == "choice":
                vals = ", ".join(f"`{c['value'] or '(empty)'}` {c['label']}" for c in s["choices"])
            elif s["kind"] == "toggle":
                vals = "`0` off, `1` on"
            elif s["kind"] == "slider":
                vals = f"{s['min']} – {s['max']} (step {s['step']})"
            else:
                vals = {"gpus": "empty = automatic, or the Vulkan device index",
                        "pads": "`auto`, `keyboard`, `none`, `pad:<SDL GUID>:<n>`"}[s["source"]]
            out.append(f"| `{s['key']}` | {s['label']} — {s['help']} | {vals} | `{s['default'] or '(empty)'}` | "
                       f"{'yes' if s.get('restart') else ''} |")
        out.append("")
    (ROOT / "docs/SETTINGS.md").write_text("\n".join(out), encoding="utf-8")
    print("wrote docs/SETTINGS.md")


def cmd_manifest(_a) -> None:
    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    MANIFEST.write_text(json.dumps(manifest(), indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {MANIFEST.relative_to(ROOT)} ({len(manifest()['settings'])} settings)")


def check(folder: Path) -> list[str]:
    bad = []
    for p in folder.rglob("*"):
        if p.is_file() and (p.suffix.lower() in FORBIDDEN or p.name.lower() in ("prod.keys", "title.keys")):
            bad.append(str(p.relative_to(folder)))
    return bad


def sha256(p: Path) -> str:
    with p.open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def cmd_windows(a) -> None:
    ver = version()
    name = f"AgeOfDecomp-v{ver}-Windows-x64"
    out = ROOT / "out/release" / name
    engine = ROOT / "out/engine/windows"
    exe = TAURI / "target/release/AgeOfDecomp.exe"
    cli = TAURI / "target/release/aod-cli.exe"
    for need in (engine / "bundle.json", exe, cli):
        if not need.is_file():
            sys.exit(f"missing {need}: run tools/engine.py stage and `pnpm tauri build --no-bundle` first")
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    shutil.copy2(exe, out / "AgeOfDecomp.exe")
    shutil.copy2(cli, out / "aod-cli.exe")
    shutil.copytree(engine, out / "engine")
    cmd_manifest(a)
    shutil.copy2(MANIFEST, out / "recomplauncher.json")
    shutil.copy2(ROOT / "LICENSE", out / "LICENSE.txt")
    lic = out / "licenses"
    lic.mkdir()
    shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", lic / "THIRD_PARTY_NOTICES.md")
    for f in (ROOT / "apps/launcher/src/assets/fonts").glob("OFL-*.txt"):
        shutil.copy2(f, lic / f.name)
    (out / "README.txt").write_text(
        f"Age of Decomp {ver} (Windows x64)\n\n"
        "A native PC build of your own Hyrule Warriors: Age of Imprisonment (Nintendo Switch 2). This package contains\n"
        "no game code or data: the game is built on this PC from your own .nup dump (base game + update 3.0.0).\n\n"
        "1. Run AgeOfDecomp.exe and follow Setup: pick your .nup, check it, choose a folder (25 GB free), Build.\n"
        "2. Press Play. Settings: in the launcher, or in game with Esc. F10 performance overlay, F11 fullscreen.\n\n"
        "RecompLauncher: this folder carries recomplauncher.json (one-click setup, settings, Big Picture).\n"
        "Updating: extract the new release over this folder; your build, settings and saves are kept.\n"
        "Licences: LICENSE.txt, licenses/ and engine/licenses/. Not affiliated with Nintendo or Koei Tecmo.\n",
        encoding="utf-8")
    bad = check(out)
    if bad:
        sys.exit("release contains forbidden files:\n" + "\n".join(bad))
    size = sum(p.stat().st_size for p in out.rglob("*") if p.is_file())
    print(f"staged {out.relative_to(ROOT)} ({size / 2**20:.0f} MiB)")
    if a.zip:
        z = out.parent / f"{name}.zip"
        if z.exists():
            z.unlink()
        with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
            for p in sorted(out.rglob("*")):
                if p.is_file():
                    zf.write(p, Path(name) / p.relative_to(out))
        sums = out.parent / "SHA256SUMS.txt"
        sums.write_text(f"{sha256(z)}  {z.name}\n", encoding="ascii")
        print(f"zip {z.relative_to(ROOT)} ({z.stat().st_size / 2**20:.0f} MiB), {sums.name}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("manifest")
    sub.add_parser("docs")
    w = sub.add_parser("windows")
    w.add_argument("--zip", action="store_true")
    c = sub.add_parser("check")
    c.add_argument("folder", type=Path)
    a = ap.parse_args()
    if a.cmd == "manifest":
        cmd_manifest(a)
    elif a.cmd == "docs":
        cmd_docs(a)
    elif a.cmd == "windows":
        cmd_windows(a)
    else:
        bad = check(a.folder)
        print("\n".join(bad) or "clean")
        sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
