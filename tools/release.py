#!/usr/bin/env python3
"""Age of Decomp release tooling (Python 3.12+, standard library only).

  py tools/release.py manifest            write packaging/recomplauncher.json from the settings catalogue
  py tools/release.py docs                write docs/SETTINGS.md from the settings catalogue
  py tools/release.py windows [--zip]     stage out/release/AgeOfDecomp-v<ver>-Windows-x64/ (+ .zip, SHA256SUMS.txt)
  python3 tools/release.py linux [--appimage] [--bin-dir D] [--dest D]
                                          stage the AppDir AgeOfDecomp-v<ver>-Linux-x64.AppDir (+ .AppImage and
                                          .tar.xz of the same folder, SHA256SUMS-Linux.txt)
  py tools/release.py check <folder>      refuse any game material in a folder (also run by `windows` and `linux`)
  py tools/release.py notes [--out F]     the GitHub release notes of VERSION: its docs/CHANGELOG.md section with the
                                          wrapped lines joined (GitHub keeps a Markdown file's line breaks)

The release holds the launcher, aod-cli, the engine bundle (out/engine/<os>: engine sources, shader translator,
Python and the whole compiler) and recomplauncher.json. It never holds anything from the game: the game is built on the
player's PC, which needs nothing installed on either system. Linux: the launcher binaries come from
packaging/linux/build-launcher.sh (Ubuntu 22.04, glibc 2.35; the frontend built first with `pnpm build`), the AppImage
from mksquashfs (squashfs-tools) + the pinned AppImage type 2 runtime (tools/engine.py PINS). Every ELF file of the
AppDir is checked: no glibc newer than LINUX_GLIBC, and no library that is neither in the folder nor on every PC.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "apps/launcher/src/lib/settings/catalog.json"
TAURI = ROOT / "apps/launcher/src-tauri"
MANIFEST = ROOT / "packaging/recomplauncher.json"
PREPARE_VERSION = (ROOT / "packaging/prepare-version.txt").read_text(encoding="ascii").strip()
BUILD_ID = "659cb288b5c5b2b49853add616c0d5bf4ea4e1ba"  # update NCM 196608 exefs/main ModuleId (ADR-005)
GAME_DIR = "{install}/game"
INI = GAME_DIR + "/work/build/hwaoi196608/hwaoi.ini"
SAVES = GAME_DIR + "/work/saves/hwaoi196608"
# Linux: the newest glibc any program or library of the AppImage may ask for (LLVM's release binaries and the
# programs built on Ubuntu 22.04 need 2.34; the game the PC builds needs the sniper sysroot's 2.31), and the libraries
# an ELF file may need from the PC: the C runtime, Vulkan's loader, and for the launcher's window GTK 3 and WebKitGTK
# 4.1 (the AppRun checks those before it opens the window; aod-cli and the game need none of them).
LINUX_GLIBC = (2, 34)
LINUX_SYSTEM_LIBS = {"libc.so.6", "libm.so.6", "libdl.so.2", "libpthread.so.0", "librt.so.1", "libutil.so.1",
                     "ld-linux-x86-64.so.2", "libgcc_s.so.1", "libstdc++.so.6", "libz.so.1", "libvulkan.so.1"}
LINUX_WINDOW_LIBS = {"libgtk-3.so.0", "libgdk-3.so.0", "libgdk_pixbuf-2.0.so.0", "libcairo.so.2", "libgobject-2.0.so.0",
                     "libglib-2.0.so.0", "libgio-2.0.so.0", "libdbus-1.so.3", "libwebkit2gtk-4.1.so.0",
                     "libjavascriptcoregtk-4.1.so.0", "libsoup-3.0.so.0", "libssl.so.3", "libcrypto.so.3"}
# Extensions that must never be in a release (game content, keys, dumps).
FORBIDDEN = {".nup", ".nsp", ".nsz", ".xci", ".xcz", ".nca", ".ncz", ".nso", ".nro", ".npdm", ".keys", ".tik", ".bfres",
             ".sarc", ".zs", ".bntx", ".bfsha", ".g1t", ".g1m", ".ktf2", ".ktsl2stbin", ".rdb", ".fdata"}
FORBIDDEN_NAMES = {"prod.keys", "title.keys", "dev.keys", "hwaoi.exe", "hwaoi"}


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
        # What the PC needs, shown on the game's page (RecompLauncher warns on an older system and never stops the
        # game): Linux's glibc is the newest any program of the AppImage asks for (LINUX_GLIBC, `release.py linux`
        # checks it), set by LLVM's binaries and the programs built on Ubuntu 22.04.
        "system": {"windows": {"version": "10", "graphics": {"vulkan": "1.3"}},
                   "linux": {"glibc": ".".join(map(str, LINUX_GLIBC)), "graphics": {"vulkan": "1.3"}}},
        # The player's copy is a named input: RecompLauncher's one way to ask for a file from 0.90 beta 68 on (its
        # top-level `game` section is deprecated there, and beta 65 and 66 refused it next to named inputs, player
        # report RL-1). Beta 68 is what players have (released 2026-11-23), so RecompLauncher looks at the picked
        # file itself: `check.nup_build_id` is the ModuleId of the newest title's exefs/main, and a copy without
        # update 1.0.3 is refused when it is picked. The setup step still checks the file first (modules and
        # resource inventory, exit 3 with the reason, shown by RecompLauncher as the setup's error).
        "inputs": [
            # Short: Easy Setup shows the label as the box's title and again under it until a file is chosen.
            {"id": "nup", "label": "Your Age of Imprisonment .nup with update 1.0.3", "role": "game",
             "formats": [{"type": "file", "extensions": ["nup"]}], "check": {"nup_build_id": BUILD_ID}},
            {"id": "dlc", "label": "Your DLC dump (.nup), if you have it", "role": "dlc", "optional": True,
             "formats": [{"type": "file", "extensions": ["nup"]}]},
            # DLAA (temporal = dlaa): on RTX PCs RecompLauncher fetches NVIDIA's nvngx_dlss.dll from NVIDIA's DLSS
            # repo, checks NVIDIA's signature and hands its path to --play (never shipped by us; other PCs skip it,
            # and the argument with {input:dlss} is left out).
            {"id": "dlss", "label": "NVIDIA DLSS", "role": "runtime", "shared": "nvidia.dlss", "optional": True},
        ],
        # The setup step is the program players start (RecompLauncher's guide: "Ship one program, and keep it the
        # one players start"): AgeOfDecomp.exe runs `cli …` without a window (main.rs, cli.rs), and the AppImage's
        # AppRun hands `cli …` to aod-cli. aod-cli.exe stays in the zip for a console and the launcher's updates.
        "prepare": {
            "program": ["AgeOfDecomp.exe", "AgeOfDecomp-x86_64.AppImage"],
            "args": ["cli", "prepare", "--game", "{input:nup}", "--out", GAME_DIR, "--dlc={input:dlc}"],
            "label": "Building the game for this PC from your copy (about 10 minutes on a fast PC)",
            # No `timeout` and no `version`: RecompLauncher 0.90 beta 68 took both out of its format. The step has
            # no time limit and runs again by itself after every new release (a run on a finished build of the same
            # engine reuses every stage: about ten seconds).
            "done": [GAME_DIR + f"/aod-ready-v{PREPARE_VERSION}.json"],
            # Frame generation (frame_gen) needs NVIDIA's nvngx_dlssg.dll, which is not ours to ship, and the shared
            # nvidia.dlss input is the DLSS file alone: the setup step fetches it from NVIDIA's DLSS repository into
            # {data}/ngx when the player ticks this (the SDK guide's "Vendor SDKs the build needs"). Ticking is the
            # player accepting NVIDIA's licence, which the second argument passes on; off, the step is as before.
            "options": [{
                "id": "frame_gen", "label": "NVIDIA frame generation",
                "note": "Downloads NVIDIA's frame generation file (7 MB) from NVIDIA. For GeForce RTX 40 and 50 cards.",
                "gpu": "nvidia", "os": ["windows"],
                "args": ["--fetch-frame-gen", "--accept-nvidia-license"],
                "license": {"name": "NVIDIA RTX SDKs license",
                            "url": "https://github.com/NVIDIA/DLSS/blob/v310.9.16/LICENSE.txt"},
            }],
        },
        # The copy in start too: the game reads the player's .nup while it plays (raw RomFS members stay in it);
        # --play checks it is the copy of the build and hands where it is now to the game (game.rs, HWAOI_NUP_PATH).
        # One argument, as the SDK guide asks.
        "start": {
            "program": ["AgeOfDecomp.exe", "AgeOfDecomp-x86_64.AppImage"],
            "args": ["--play", "--game-dir", GAME_DIR, "--game={input:nup}", "--dlss={input:dlss}"],
        },
        # `new`: the engine's own first hwaoi.ini (compiler/backend.py), used if RecompLauncher has to create the file
        # (a setting changed before setup): the setup step moves such an early file aside and merges it back after
        # the build (prepare.rs set_aside_early_files), because the engine only builds into an empty folder.
        "config": [{"id": "main", "format": "ini", "file": [{"path": INI}],
                    "new": {"output_res": "auto", "render_scale": "100", "fps_limit": "60"}}],
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
            "64": "Build or rebuild the game for this version. Press Set Up Again.",
            "65": "Your .nup is missing or isn't the copy the game was built from. Pick that copy, or set up again.",
            # the runtime's rt_fatal (dispatch.c) and crash handler (main.c); the log's end is also on stderr (--play)
            "70": "The game stopped on an error. Its log says why: send the newest game-*.log from Logs with your report.",
            "71": "The game crashed. Send the newest game-*.log from Logs with your report.",
        },
        # RecompLauncher's looks (its Shader tab: ReShade filters it adds for the game's process). The game draws with
        # Vulkan, which RecompLauncher cannot work out from the start program: AgeOfDecomp.exe is the launcher's
        # window and starts the game, which takes the Vulkan layer from the environment. On for a first test
        # (2026-11-23): the same renderer in TotK PC Recomp runs them; not yet seen here, with frame generation or HDR
        # least of all (the looks leave an HDR picture alone).
        "looks": {"graphics": "vulkan"},
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
    (ROOT / "docs/SETTINGS.md").write_text("\n".join(out), encoding="utf-8", newline="\n")
    print("wrote docs/SETTINGS.md")


def cmd_manifest(_a) -> None:
    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    MANIFEST.write_text(json.dumps(manifest(), indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {MANIFEST.relative_to(ROOT)} ({len(manifest()['settings'])} settings)")


def check(folder: Path) -> list[str]:
    bad = []
    for p in folder.rglob("*"):
        if p.is_file() and (p.suffix.lower() in FORBIDDEN or p.name.lower() in FORBIDDEN_NAMES):
            bad.append(str(p.relative_to(folder)))
    return bad


def sha256(p: Path) -> str:
    with p.open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def verify_engine(engine: Path) -> int:
    """The copied engine must be exactly what `engine.py stage` hashed into bundle.json: nothing added after staging
    (logs, saves, generated C, a test's leftovers), nothing missing or changed. Python bytecode caches, which running
    the staged Python writes, are dropped from the copy. Returns the number of files."""
    files = json.loads((engine / "bundle.json").read_text(encoding="utf-8"))["files"]
    for cache in sorted(engine.rglob("__pycache__"), reverse=True):
        if cache.is_dir() and not cache.is_symlink():
            shutil.rmtree(cache)
    found = {p.relative_to(engine).as_posix() for p in engine.rglob("*") if p.is_file()} - {"bundle.json"}
    problems = ([f"added after staging: {rel}" for rel in sorted(found - files.keys())] +
                [f"missing: {rel}" for rel in sorted(files.keys() - found)] +
                [f"changed: {rel}" for rel in sorted(found & files.keys()) if sha256(engine / rel) != files[rel]])
    if problems:
        sys.exit("the engine bundle differs from its bundle.json; run tools/engine.py stage again:\n" +
                 "\n".join(problems[:40]) + (f"\n… {len(problems) - 40} more" if len(problems) > 40 else ""))
    return len(files)


def verify_zip(z: Path, folder: Path, top: str) -> None:
    """The archive holds exactly the release folder, and every member reads back with its CRC."""
    want = {f"{top}/{p.relative_to(folder).as_posix()}": p.stat().st_size for p in folder.rglob("*") if p.is_file()}
    with zipfile.ZipFile(z) as zf:
        infos = [i for i in zf.infolist() if not i.filename.endswith("/")]
        broken = zf.testzip()
    names = [i.filename for i in infos]
    sizes = {i.filename: i.file_size for i in infos}
    if len(names) != len(set(names)) or sizes != want or broken:
        sys.exit(f"{z.name} does not match {folder.name}: {len(set(sizes.items()) ^ set(want.items()))} differing "
                 f"entries, {len(names) - len(set(names))} repeated, first bad member {broken}")


def stale_engine_files(engine: Path) -> list[str]:
    """The repository's engine files that the staged bundle does not carry as they are now (VERSION is the bundle's
    own: the stage writes the release's number there)."""
    sys.path.insert(0, str(ROOT / "tools"))   # `py -I` leaves the script's folder out of the path
    import engine as engine_tool   # tools/engine.py: the same list of files and the same hash as its stage

    staged = json.loads((engine / "bundle.json").read_text(encoding="utf-8")).get("files", {})
    current = engine_tool.source_files()
    # Both ways: a file the repository no longer has, still in the bundle, makes it stale as well
    gone = sorted({p[len("source/"):] for p in staged if p.startswith("source/")} - {*current, "VERSION"})
    return [name for name in current
            if name != "VERSION" and staged.get(f"source/{name}") != engine_tool.sha256(engine_tool.ENGINE / name)] + gone


def release_info(allow_dirty: bool, engine: Path) -> dict:
    def git(*args):
        return subprocess.run(["git", *args], cwd=ROOT, check=True, capture_output=True, text=True).stdout.strip()

    dirty = bool(git("status", "--porcelain"))
    if dirty and not allow_dirty:
        sys.exit("repository has uncommitted changes; commit them or pass --allow-dirty")
    if dirty:
        print("releasing a dirty repository (--allow-dirty)")
    bundle = json.loads((engine / "bundle.json").read_text(encoding="utf-8"))
    # A clean repository is not enough: the bundle may have been staged while engine files were uncommitted, or
    # before the last engine change. A release carries the engine of its commit.
    if bundle.get("dirty") and not allow_dirty:
        sys.exit("the engine bundle was staged from uncommitted engine files; run tools/engine.py stage again "
                 "(or pass --allow-dirty)")
    stale = stale_engine_files(engine)
    if stale and not allow_dirty:
        sys.exit(f"the engine bundle is not this repository's engine: {len(stale)} file(s) differ, the first is "
                 f"{stale[0]}; run tools/engine.py stage again (or pass --allow-dirty)")
    if stale:
        print(f"releasing an engine bundle that differs from the repository in {len(stale)} file(s) (--allow-dirty)")
    runtime_version = (engine / "source/VERSION").read_text(encoding="utf-8").strip()
    if bundle.get("version") != version() or runtime_version != version():
        sys.exit("engine/compiler versions differ; run tools/engine.py stage again")
    if bundle.get("aod_prepare_version") != int(PREPARE_VERSION):
        sys.exit("engine/setup versions differ; run tools/engine.py stage again")
    return {"compiler_version": version(), "repository_revision": git("rev-parse", "HEAD"), "engine_source_revision": bundle["source_revision"],
            "dirty": dirty, "engine_dirty": bool(bundle.get("dirty", False))}


def cmd_windows(a) -> None:
    ver = version()
    name = f"AgeOfDecomp-v{ver}-Windows-x64"
    out = ROOT / "out/release" / name
    engine = ROOT / "out/engine/windows"
    # Use the Cargo binary from the same build as aod-cli. Tauri's optional
    # bundling step may have left an older renamed AgeOfDecomp.exe beside it.
    exe = TAURI / "target/release/age-of-decomp.exe"
    cli = TAURI / "target/release/aod-cli.exe"
    for need in (engine / "bundle.json", exe, cli):
        if not need.is_file():
            sys.exit(f"missing {need}: stage the engine, run `pnpm build` in apps/launcher, then "
                     "`cargo build --locked --release --bins --features tauri/custom-protocol` in src-tauri")
    info = release_info(a.allow_dirty, engine)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    shutil.copy2(exe, out / "AgeOfDecomp.exe")
    shutil.copy2(cli, out / "aod-cli.exe")
    shutil.copytree(engine, out / "engine")
    print(f"engine: {verify_engine(out / 'engine')} files match bundle.json")
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
        "no game code or data: the game is built on this PC from your own .nup dump (the game with update 1.0.3).\n\n"
        "1. Run AgeOfDecomp.exe and follow Setup: pick your .nup, check it, choose a folder (25 GB free), Build.\n"
        "2. Press Play. Settings: in the launcher, or in game with Esc. F10 performance overlay, F11 fullscreen.\n\n"
        "RecompLauncher: this folder carries recomplauncher.json (one-click setup, settings, Big Picture).\n"
        "Updating: use System > Updates in Age of Decomp. Rebuild through Setup after an engine update.\n"
        "Manual updates: extract the new release over this folder; your build, settings and saves are kept.\n"
        "Command line: `AgeOfDecomp.exe cli prepare|play|verify …` (RecompLauncher's setup step), or aod-cli.exe\n"
        "with the same commands in a console.\n"
        "Licences: LICENSE.txt, licenses/ and engine/licenses/. Not affiliated with Nintendo or Koei Tecmo.\n",
        encoding="utf-8")
    (out / "build-info.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8", newline="\n")
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
        verify_zip(z, out, name)
        sums = out.parent / "SHA256SUMS.txt"
        # LF line ends: `sha256sum -c` on Linux reads a CRLF line's name with the CR and finds no file.
        sums.write_text(f"{sha256(z)}  {z.name}\n", encoding="ascii", newline="\n")
        print(f"zip {z.relative_to(ROOT)} ({z.stat().st_size / 2**20:.0f} MiB), {sums.name}")


def cmd_linux(a) -> None:
    """The AppImage's AppDir: AppRun (a leading `cli` goes to aod-cli, anything else to the launcher), the launcher,
    aod-cli, engine/ (out/engine/linux), recomplauncher.json, the .desktop file and icon, licences."""
    ver = version()
    name = f"AgeOfDecomp-v{ver}-Linux-x64"
    dest = Path(a.dest).expanduser().resolve() if a.dest else ROOT / "out/release"
    out = dest / f"{name}.AppDir"
    engine = ROOT / "out/engine/linux"
    bins = Path(a.bin_dir).expanduser() if a.bin_dir else TAURI / "target/release"
    exe = bins / "age-of-decomp"
    cli = bins / "aod-cli"
    for need in (engine / "bundle.json", exe, cli):
        if not need.is_file():
            sys.exit(f"missing {need}: run tools/engine.py stage and build the launcher on Linux first "
                     "(pnpm build; cargo build --release --features tauri/custom-protocol)")
    info = release_info(a.allow_dirty, engine)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    linux = ROOT / "packaging/linux"
    for src, dst in ((exe, "AgeOfDecomp"), (cli, "aod-cli"), (linux / "AppRun", "AppRun")):
        shutil.copyfile(src, out / dst)
        os.chmod(out / dst, 0o755)
    shutil.copytree(engine, out / "engine", symlinks=True)
    print(f"engine: {verify_engine(out / 'engine')} files match bundle.json")
    shutil.copyfile(linux / "age-of-decomp.desktop", out / "age-of-decomp.desktop")
    shutil.copyfile(TAURI / "icons/128x128@2x.png", out / "age-of-decomp.png")   # 256x256
    (out / ".DirIcon").symlink_to("age-of-decomp.png")
    cmd_manifest(a)
    shutil.copy2(MANIFEST, out / "recomplauncher.json")
    shutil.copy2(ROOT / "LICENSE", out / "LICENSE.txt")
    lic = out / "licenses"
    lic.mkdir()
    shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", lic / "THIRD_PARTY_NOTICES.md")
    for f in (ROOT / "apps/launcher/src/assets/fonts").glob("OFL-*.txt"):
        shutil.copy2(f, lic / f.name)
    (out / "README.txt").write_text(
        f"Age of Decomp {ver} (Linux x86-64 AppImage)\n\n"
        "A native PC build of your own Hyrule Warriors: Age of Imprisonment (Nintendo Switch 2). This package contains\n"
        "no game code or data: the game is built on this PC from your own .nup dump (the game with update 1.0.3).\n\n"
        "Needs a 64-bit Linux with glibc 2.34 or newer (SteamOS 3, Ubuntu 22.04, Debian 12, Fedora 36 or newer) and a\n"
        "Vulkan 1.3 driver (Mesa or NVIDIA); Linux 6.7 or later is faster. Nothing to install: the compiler that\n"
        "builds the game on this PC is inside. Only the launcher's own window needs WebKitGTK 4.1 from the system\n"
        "(Debian/Ubuntu libwebkit2gtk-4.1-0, Fedora webkit2gtk4.1, Arch webkit2gtk-4.1). On a Steam Deck, or without\n"
        "it, set up and play with RecompLauncher, which runs the steps below without that window.\n\n"
        "1. Run the AppImage and follow Setup: pick your .nup, check it, choose a folder (25 GB free), Build.\n"
        "2. Press Play. Settings: in the launcher, or in game with Esc. F10 performance overlay, F11 fullscreen.\n\n"
        "Command line: `AgeOfDecomp-…AppImage cli prepare|play|verify …` and `--play` run aod-cli (RecompLauncher\n"
        "uses them). Licences: LICENSE.txt, licenses/ and engine/licenses/. Not affiliated with Nintendo or Koei Tecmo.\n",
        encoding="utf-8")
    (out / "build-info.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8", newline="\n")
    bad = check(out)
    if bad:
        sys.exit("release contains forbidden files:\n" + "\n".join(bad))
    check_linux_programs(out)
    size = sum(p.stat().st_size for p in out.rglob("*") if p.is_file() and not p.is_symlink())
    print(f"staged {out} ({size / 2**20:.0f} MiB)")
    if a.appimage:
        # An AppImage (type 2) is the runtime ELF followed by a squashfs of the AppDir, which is all appimagetool does
        # without update information or signatures: mksquashfs (squashfs-tools) + the pinned runtime.
        if not shutil.which("mksquashfs"):
            sys.exit("mksquashfs not found: install squashfs-tools")
        sys.path.insert(0, str(ROOT / "tools"))
        import engine as eng   # the pinned downloads (tools/engine.py PINS)
        runtime = eng.fetch("appimage-runtime")
        image = dest / f"{name}.AppImage"
        squash = dest / f"{name}.squashfs"
        for old in (image, squash):
            if old.exists():
                old.unlink()
        subprocess.run(["mksquashfs", str(out), str(squash), "-root-owned", "-noappend", "-quiet",
                        "-comp", "zstd", "-Xcompression-level", "19", "-b", "1M"], check=True)
        with image.open("wb") as f:
            f.write(runtime.read_bytes())
            with squash.open("rb") as s:
                shutil.copyfileobj(s, f, 1 << 20)
        squash.unlink()
        os.chmod(image, 0o755)
        tarball = dest / f"{name}.tar.xz"
        pack_tarball(out, tarball, name)
        sums = dest / "SHA256SUMS-Linux.txt"
        sums.write_text(f"{sha256(image)}  {image.name}\n{sha256(tarball)}  {tarball.name}\n", encoding="ascii",
                        newline="\n")
        print(f"AppImage {image} ({image.stat().st_size / 2**20:.0f} MiB), {tarball.name} "
              f"({tarball.stat().st_size / 2**20:.0f} MiB), {sums.name}")


def release_notes(changelog: str, ver: str) -> str:
    """The section `## <ver> (…)` of the changelog, its lines joined the way GitHub's release page needs them: a list
    item or a paragraph on one line (the changelog wraps them at 120 columns; GitHub keeps those breaks)."""
    lines = changelog.splitlines()
    start = next((i for i, line in enumerate(lines) if line.startswith(f"## {ver} ")), None)
    if start is None:
        sys.exit(f"docs/CHANGELOG.md has no section for {ver}")
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith("## ")), len(lines))
    out: list[str] = []
    for line in lines[start + 1:end]:
        joinable = out and out[-1] and not out[-1].startswith("#")
        if line.startswith("  ") and line.strip() and joinable:   # a wrapped list item goes on
            out[-1] += " " + line.strip()
        elif line and not line.startswith(("#", "- ", "  ")) and joinable and not out[-1].startswith("- "):
            out[-1] += " " + line.strip()                             # a wrapped paragraph goes on
        else:
            out.append(line.rstrip())
    while out and not out[0]:
        out.pop(0)
    while out and not out[-1]:
        out.pop()
    return "\n".join(out) + "\n"


def cmd_notes(a) -> None:
    text = release_notes((ROOT / "docs/CHANGELOG.md").read_text(encoding="utf-8"), version())
    if a.out:
        Path(a.out).write_text(text, encoding="utf-8", newline="\n")
        print(f"wrote {a.out}")
    else:
        sys.stdout.write(text)


def elf_info(path: Path) -> tuple[list[str], list[str], tuple[int, ...]]:
    """An ELF file's DT_NEEDED libraries, its RUNPATH/RPATH folders and the newest GLIBC_ symbol version it asks for
    (binutils' readelf)."""
    import re
    dyn = subprocess.run(["readelf", "-d", "-W", str(path)], capture_output=True, text=True, check=True).stdout
    ver = subprocess.run(["readelf", "-V", "-W", str(path)], capture_output=True, text=True, check=True).stdout
    needed = re.findall(r"\(NEEDED\)\s+Shared library: \[([^\]]+)\]", dyn)
    runpath = [d for found in re.findall(r"\((?:RUNPATH|RPATH)\)\s+Library r(?:un)?path: \[([^\]]*)\]", dyn)
               for d in found.split(":") if d]
    newest = max((tuple(map(int, v.split("."))) for v in re.findall(r"GLIBC_(\d+(?:\.\d+)+)", ver)), default=(0,))
    return needed, runpath, newest


def check_linux_programs(out: Path) -> None:
    """Every ELF file of the AppDir (the sysroot aside: glibc itself, which only the build's link reads) asks for no
    glibc newer than LINUX_GLIBC, and each library it needs is one every PC has or one the loader finds in the folder
    through the file's own RUNPATH ($ORIGIN), as the loader would; only the launcher's window may need GTK and
    WebKitGTK. The two programs must be x86-64 Linux executables, and no link may lead outside the folder."""
    if not shutil.which("readelf"):
        sys.exit("readelf not found: install binutils")
    for name in ("AgeOfDecomp", "aod-cli"):
        head = (out / name).read_bytes()[:20]
        if head[:4] != b"\x7fELF" or head[4] != 2 or int.from_bytes(head[18:20], "little") != 62:
            sys.exit(f"{name} is not an x86-64 Linux program: build it with packaging/linux/build-launcher.sh")
    sysroot = out / "engine" / "llvm" / "sysroot"
    newest, by, problems, count = (0,), [], [], 0
    for p in sorted(out.rglob("*")):
        if p.is_symlink():
            target = p.resolve()
            if not target.exists() or not target.is_relative_to(out.resolve()):
                problems.append(f"{p.relative_to(out)} links to {os.readlink(p)}, outside the folder or to nothing")
            continue
        if not p.is_file() or sysroot in p.parents:
            continue
        with p.open("rb") as f:
            if f.read(4) != b"\x7fELF":
                continue
        count += 1
        needed, runpath, need = elf_info(p)
        dirs = [Path(d.replace("$ORIGIN", str(p.parent)).replace("${ORIGIN}", str(p.parent))) for d in runpath]
        system = LINUX_SYSTEM_LIBS | (LINUX_WINDOW_LIBS if p.name == "AgeOfDecomp" else set())
        for lib in needed:
            if "/" in lib:   # a path the loader opens as it is, $ORIGIN expanded
                found = Path(lib.replace("$ORIGIN", str(p.parent)).replace("${ORIGIN}", str(p.parent))).is_file()
            else:
                found = lib in system or any((d / lib).is_file() for d in dirs)
            if not found:
                problems.append(f"{p.relative_to(out)} needs {lib}, neither on every PC nor in its RUNPATH")
        if need > newest:
            newest, by = need, []
        if need == newest:
            by.append(p.relative_to(out).as_posix())
    floor = ".".join(map(str, LINUX_GLIBC))
    if newest > LINUX_GLIBC:
        problems.append(f"{', '.join(by[:6])} need glibc {'.'.join(map(str, newest))}, above {floor}")
    if problems:
        sys.exit("the Linux programs need more than the PCs they are for:\n" + "\n".join(problems[:30]))
    print(f"programs: {count} ELF files need glibc {'.'.join(map(str, newest))} at most (floor {floor}: "
          f"{', '.join(by[:4])}{' and others' if len(by) > 4 else ''}) and no library a PC may lack")


def pack_tarball(folder: Path, tarball: Path, top: str) -> None:
    """`folder` as `top/` in a .tar.xz, owned by root like the AppImage, modes and symlinks kept. RecompLauncher's SDK
    guide asks for one portable Linux folder packed both ways: it unpacks the AppImage and plays from a plain folder
    (AppRun, APPIMAGE unset), and installs the tarball when an AppImage will not unpack."""
    def owner(ti: tarfile.TarInfo) -> tarfile.TarInfo:
        ti.uid = ti.gid = 0
        ti.uname = ti.gname = "root"
        return ti
    if tarball.exists():
        tarball.unlink()
    with tarfile.open(tarball, "w:xz") as t:
        t.add(folder, arcname=top, filter=owner)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("manifest")
    sub.add_parser("docs")
    w = sub.add_parser("windows")
    w.add_argument("--zip", action="store_true")
    w.add_argument("--allow-dirty", action="store_true")
    li = sub.add_parser("linux")
    li.add_argument("--allow-dirty", action="store_true")
    li.add_argument("--appimage", action="store_true", help="also pack the AppImage (mksquashfs + pinned runtime) and a .tar.xz of the same folder")
    li.add_argument("--bin-dir", help="folder with the Linux launcher binaries (default src-tauri/target/release)")
    li.add_argument("--dest", help="output folder (default out/release; under WSL use the Linux file system)")
    c = sub.add_parser("check")
    c.add_argument("folder", type=Path)
    n = sub.add_parser("notes")
    n.add_argument("--out", help="write the notes to this file instead of standard output")
    a = ap.parse_args()
    if a.cmd == "manifest":
        cmd_manifest(a)
    elif a.cmd == "docs":
        cmd_docs(a)
    elif a.cmd == "windows":
        cmd_windows(a)
    elif a.cmd == "linux":
        cmd_linux(a)
    elif a.cmd == "notes":
        cmd_notes(a)
    else:
        if not a.folder.is_dir():
            sys.exit(f"{a.folder} is not a folder")
        bad = check(a.folder)
        print("\n".join(bad) or "clean")
        sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
