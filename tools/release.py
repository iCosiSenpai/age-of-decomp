#!/usr/bin/env python3
"""Age of Decomp release tooling (Python 3.12+, standard library only).

  py tools/release.py manifest            write packaging/recomplauncher.json from the settings catalogue
  py tools/release.py docs                write docs/SETTINGS.md from the settings catalogue
  py tools/release.py windows [--zip]     stage out/release/AgeOfDecomp-v<ver>-Windows-x64/ (+ .zip, SHA256SUMS.txt)
  python3 tools/release.py linux [--appimage] [--bin-dir D] [--dest D]
                                          stage the AppDir AgeOfDecomp-v<ver>-Linux-x64.AppDir (+ .AppImage and
                                          .tar.xz of the same folder, SHA256SUMS-Linux.txt)
  py tools/release.py check <folder>      refuse any game material in a folder (also run by `windows` and `linux`)

The release holds the launcher, aod-cli, the engine bundle (out/engine/<os>: engine sources + shader translator +
Python, on Windows also the compiler) and recomplauncher.json. It never holds anything from the game: the game is built
on the player's PC. Linux: the launcher binaries come from `cargo build --release --features tauri/custom-protocol`
(frontend built first with `pnpm build`), the AppImage from mksquashfs (squashfs-tools) + the pinned AppImage type 2
runtime (tools/engine.py PINS); on Linux the player needs clang, lld, Ninja and the SDL3/Opus/libvpx/zstd/Vulkan/zlib
development packages.
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
        # What the PC needs, shown on the game's page (RecompLauncher warns on an older Windows and never stops the
        # game). Only the systems a release ships a build for: add "linux" (glibc of the build system, Vulkan 1.3)
        # with the first published Linux build.
        "system": {"windows": {"version": "10", "graphics": {"vulkan": "1.3"}}},
        # The player's copy is a named input, not a `game` section. RecompLauncher's Easy Setup (0.90 beta 65 and 66,
        # ui/setup-steps.js stepsRun) runs a profile with named inputs through setup_inputs_run, which refuses a
        # `game` profile whose copy was only picked in that window ("That copy didn't work: Choose your copy of the
        # game first", player report RL-1). Named inputs carry no content check, so `aod-cli prepare` checks the
        # build ID itself first (exit 3 with the reason, shown by RecompLauncher as the setup's error).
        "inputs": [
            # Short: Easy Setup shows the label as the box's title and again under it until a file is chosen.
            {"id": "nup", "label": "Your Age of Imprisonment .nup with update 3.0.0", "role": "game",
             "formats": [{"type": "file", "extensions": ["nup"]}]},
            {"id": "dlc", "label": "Your DLC dump (.nup), if you have it", "role": "dlc", "optional": True,
             "formats": [{"type": "file", "extensions": ["nup"]}]},
            # DLAA (temporal = dlaa): on RTX PCs RecompLauncher fetches NVIDIA's nvngx_dlss.dll from NVIDIA's DLSS
            # repo, checks NVIDIA's signature and hands its path to --play (never shipped by us; other PCs skip it,
            # and the argument with {input:dlss} is left out).
            {"id": "dlss", "label": "NVIDIA DLSS", "role": "runtime", "shared": "nvidia.dlss", "optional": True},
        ],
        "prepare": {
            "program": ["aod-cli.exe", "AgeOfDecomp-x86_64.AppImage"],
            "args": ["cli", "prepare", "--game", "{input:nup}", "--out", GAME_DIR, "--dlc={input:dlc}"],
            "label": "Building the game for this PC from your copy (about 10 minutes on a fast PC)",
            "timeout": 14400,
            "version": PREPARE_VERSION,
            "done": [GAME_DIR + f"/aod-ready-v{PREPARE_VERSION}.json"],
        },
        # The copy in start too: the game reads the player's .nup while it plays (raw RomFS members stay in it);
        # --play checks it is the copy of the build and hands where it is now to the game (game.rs, HWAOI_NUP_PATH).
        # One argument, as the SDK guide asks.
        "start": {
            "program": ["AgeOfDecomp.exe", "AgeOfDecomp-x86_64.AppImage"],
            "args": ["--play", "--game-dir", GAME_DIR, "--game={input:nup}", "--dlss={input:dlss}"],
        },
        # `new`: the engine's own first hwaoi.ini (compiler/backend.py), used if RecompLauncher has to create the file
        # (a setting changed before setup): aod-cli prepare moves such an early file aside and merges it back after
        # the build (prepare.rs stash_early_settings), because the engine only builds into an empty folder.
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
    want = {f"{top}/{p.relative_to(folder).as_posix()}" for p in folder.rglob("*") if p.is_file()}
    with zipfile.ZipFile(z) as zf:
        names = {n for n in zf.namelist() if not n.endswith("/")}
        broken = zf.testzip()
    if names != want or broken:
        sys.exit(f"{z.name} does not match {folder.name}: {len(names ^ want)} differing entries, first bad member {broken}")


def release_info(allow_dirty: bool, engine: Path) -> dict:
    def git(*args):
        return subprocess.run(["git", *args], cwd=ROOT, check=True, capture_output=True, text=True).stdout.strip()

    dirty = bool(git("status", "--porcelain"))
    if dirty and not allow_dirty:
        sys.exit("repository has uncommitted changes; commit them or pass --allow-dirty")
    if dirty:
        print("releasing a dirty repository (--allow-dirty)")
    bundle = json.loads((engine / "bundle.json").read_text(encoding="utf-8"))
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
        "no game code or data: the game is built on this PC from your own .nup dump (base game + update 3.0.0).\n\n"
        "1. Run AgeOfDecomp.exe and follow Setup: pick your .nup, check it, choose a folder (25 GB free), Build.\n"
        "2. Press Play. Settings: in the launcher, or in game with Esc. F10 performance overlay, F11 fullscreen.\n\n"
        "RecompLauncher: this folder carries recomplauncher.json (one-click setup, settings, Big Picture).\n"
        "Updating: use System > Updates in the compiler. Rebuild through Setup after an engine update.\n"
        "Manual updates: extract the new release over this folder; your build, settings and saves are kept.\n"
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
        "no game code or data: the game is built on this PC from your own .nup dump (base game + update 3.0.0).\n\n"
        "Needs Linux 6.7 or later (older kernels work, slower), a Vulkan 1.3 driver with VK_EXT_external_memory_host,\n"
        "WebKitGTK 4.1 for the launcher window, and to build the game: clang, lld, Ninja and the SDL3, Opus, libvpx,\n"
        "zstd, Vulkan and zlib development packages. Debian/Ubuntu:\n"
        "  sudo apt install clang lld ninja-build libsdl3-dev libopus-dev libvpx-dev libzstd-dev libvulkan-dev zlib1g-dev\n\n"
        "1. Run the AppImage and follow Setup: pick your .nup, check it, choose a folder (25 GB free), Build.\n"
        "2. Press Play. Settings: in the launcher, or in game with Esc. F10 performance overlay, F11 fullscreen.\n\n"
        "Command line: `AgeOfDecomp-…AppImage cli prepare|play|verify …` runs aod-cli (RecompLauncher uses it).\n"
        "Licences: LICENSE.txt, licenses/ and engine/licenses/. Not affiliated with Nintendo or Koei Tecmo.\n",
        encoding="utf-8")
    (out / "build-info.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8", newline="\n")
    bad = check(out)
    if bad:
        sys.exit("release contains forbidden files:\n" + "\n".join(bad))
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
    a = ap.parse_args()
    if a.cmd == "manifest":
        cmd_manifest(a)
    elif a.cmd == "docs":
        cmd_docs(a)
    elif a.cmd == "windows":
        cmd_windows(a)
    elif a.cmd == "linux":
        cmd_linux(a)
    else:
        bad = check(a.folder)
        print("\n".join(bad) or "clean")
        sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
