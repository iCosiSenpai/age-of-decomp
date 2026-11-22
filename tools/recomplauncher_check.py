#!/usr/bin/env python3
"""Runs RecompLauncher's own Developer-tab checks on an Age of Decomp release folder, without installing RecompLauncher.

  py tools/recomplauncher_check.py [--release out/release/AgeOfDecomp-v<ver>-Windows-x64] [--nup <player's .nup>]

How: a git worktree of refs/RecompLauncher (`py tools/fetch_refs.py`) in .cache/rlcheck/src, with
tools/recomplauncher_check.rs appended to its src-tauri/src/devtools.rs, then `cargo test --lib aod_check`. Their code
is compiled, never changed in refs/. Windows needs Visual Studio's C++ tools (their sdl3-sys builds SDL with MSVC via
CMake + Ninja inside vcvars64) and a short target folder (.cache/rlt: cl.exe cannot open paths over 260 characters).
The first run compiles their whole app (~10 min); later runs are incremental. Exit 0 = no errors in their checks.
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "refs" / "RecompLauncher"
WORK = ROOT / ".cache" / "rlcheck" / "src"
TARGET = ROOT / ".cache" / "rlt"
HARNESS = Path(__file__).with_suffix(".rs")
MARK = "// Age of Decomp harness appended by tools/recomplauncher_check.py"


def git(*args: str, cwd: Path = REF) -> str:
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def prepare_worktree() -> str:
    if not REF.is_dir():
        sys.exit("refs/RecompLauncher is missing: run `py tools/fetch_refs.py` first")
    head = git("rev-parse", "HEAD")
    if not (WORK / ".git").exists():
        WORK.parent.mkdir(parents=True, exist_ok=True)
        git("worktree", "prune")
        git("worktree", "add", "--detach", str(WORK), head)
    else:
        git("checkout", "--detach", "-f", head, cwd=WORK)
        git("checkout", "--", ".", cwd=WORK)   # drop a previous harness
    devtools = WORK / "src-tauri" / "src" / "devtools.rs"
    text = devtools.read_text(encoding="utf-8")
    if MARK not in text:
        devtools.write_text(text + "\n" + HARNESS.read_text(encoding="utf-8"), encoding="utf-8")
    return head[:10]


def vcvars() -> Path | None:
    hits = glob.glob(r"C:\Program Files\Microsoft Visual Studio\*\*\VC\Auxiliary\Build\vcvars64.bat")
    return Path(sorted(hits)[-1]) if hits else None


def cmake() -> str | None:
    found = shutil.which("cmake")
    if found:
        return found
    hits = glob.glob(r"C:\Program Files\Microsoft Visual Studio\*\*\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe")
    return sorted(hits)[-1] if hits else None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ver = json.loads((ROOT / "apps/launcher/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["version"]
    # The folder tools/release.py makes: the unpacked zip on Windows, the AppDir on Linux.
    name = f"AgeOfDecomp-v{ver}-Windows-x64" if os.name == "nt" else f"AgeOfDecomp-v{ver}-Linux-x64.AppDir"
    ap.add_argument("--release", type=Path, default=ROOT / "out" / "release" / name)
    ap.add_argument("--nup", type=Path, help="also read this .nup with RecompLauncher's own NUP reader")
    args = ap.parse_args()
    release = args.release.resolve()
    if not (release / "recomplauncher.json").is_file():
        sys.exit(f"{release} has no recomplauncher.json: run tools/release.py first")
    rev = prepare_worktree()
    # The manifest asks for the copy as a named input (no content check there), so the build the setup step
    # accepts comes from the release tool.
    found = re.search(r'^BUILD_ID = "([0-9a-f]+)"', (ROOT / "tools/release.py").read_text(encoding="utf-8"), re.M)
    if not found:
        sys.exit("tools/release.py has no BUILD_ID")
    env = dict(os.environ, AOD_RELEASE=str(release), CARGO_TARGET_DIR=str(TARGET), AOD_BUILD_ID=found.group(1))
    if args.nup:
        env["AOD_NUP"] = str(args.nup.resolve())
    test = ["cargo", "test", "--lib", "aod_check", "--", "--nocapture", "--test-threads=1"]
    crate = WORK / "src-tauri"
    print(f"RecompLauncher {rev} checks on {release}", flush=True)
    if os.name == "nt":
        bat, cm = vcvars(), cmake()
        if not bat or not cm:
            sys.exit("Visual Studio with the C++ tools (vcvars64.bat, CMake) is needed to build RecompLauncher's sdl3-sys")
        ninja = ROOT / ".cache" / "toolchain" / "windows" / "bin"
        env.update(CMAKE_GENERATOR="Ninja", CMAKE=cm, PATH=f"{ninja}{os.pathsep}{env.get('PATH', '')}")
        with tempfile.NamedTemporaryFile("w", suffix=".cmd", delete=False, encoding="utf-8") as f:
            f.write(f'@echo off\r\ncall "{bat}" >nul\r\ncd /d "{crate}"\r\n{subprocess.list2cmdline(test)}\r\n')
        cmd = ["cmd", "/d", "/c", f.name]
    else:
        cmd = test
    r = subprocess.run(cmd, cwd=crate, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")
    if os.name == "nt":
        os.unlink(f.name)
    out = (r.stdout or "") + (r.stderr or "")
    lines = [l for l in out.splitlines() if l.startswith("AOD ") or "test result" in l or "panicked" in l]
    print("\n".join(lines) if lines else out[-4000:])
    errors = sum(1 for l in lines if l.startswith("AOD [err]"))
    warns = sum(1 for l in lines if l.startswith("AOD [warn]"))
    print(f"=> exit {r.returncode}: {errors} error(s), {warns} warning(s)")
    return r.returncode


if __name__ == "__main__":
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(errors="replace")
    raise SystemExit(main())
