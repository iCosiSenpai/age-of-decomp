#!/usr/bin/env python3
"""Age of Decomp engine tooling (Python 3.12+, standard library only; Windows x64 and Linux x64).

  py tools/engine.py toolchain         pinned tools and libraries  -> .cache/toolchain/<os>/
  py tools/engine.py fetch [--force]   engine checkout at engine/upstream.json + engine/patches -> .cache/engine/hwaoi-recomp
  py tools/engine.py export-patches    commits on branch `aod` above the pin -> engine/patches/*.patch
  py tools/engine.py translator        Ryujinx-based shader translator (self-contained .NET publish)
  py tools/engine.py stage             portable engine bundle -> out/engine/<os>/ (format hwaoi-compiler-v1)
  py tools/engine.py game --nup F [--out D]   developer build of the game from a NUP with the staged bundle
  py tools/engine.py status            what is present

Every download is pinned by URL and SHA-256 (pins shared with iCosiSenpai/totk-pc-recomp tools/make_release.py, MIT).
Nothing is installed system-wide. No game content is ever written outside ignored folders.
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
import time
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WIN = os.name == "nt"
OSNAME = "windows" if WIN else "linux"
CACHE = ROOT / ".cache"
DL = CACHE / "downloads"
TC = CACHE / "toolchain" / OSNAME
UNPACK = CACHE / "unpacked"
ENGINE = CACHE / "engine" / "hwaoi-recomp"
STAGE = ROOT / "out" / "engine" / OSNAME
UPSTREAM = ROOT / "engine" / "upstream.json"
PATCHES = ROOT / "engine" / "patches"
OWN_SRC = ROOT / "engine" / "src" / "aod"
XLATE_OUT = ENGINE / "work" / "build" / "hwaoi196608" / "xlate-managed-irbanks"  # build_xlate_hwaoi.py requires this parent

MSYS2 = "https://repo.msys2.org/mingw/ucrt64/"
SDL_VERSION = "3.4.20"
LLVM_VERSION = "20261020"
PY_VERSION = "3.14.9"
PINS = {  # name: (url, sha256)
    "python": (f"https://api.nuget.org/v3-flatcontainer/python/{PY_VERSION}/python.{PY_VERSION}.nupkg",
               "7cd9d07aec975212344dc6f7e45019378cf6775dd564181e269010a1eddaaec7"),
    "llvm-mingw": (f"https://github.com/mstorsjo/llvm-mingw/releases/download/{LLVM_VERSION}/"
                   f"llvm-mingw-{LLVM_VERSION}-ucrt-x86_64.zip",
                   "fd5462b7f5d93b2ff981fcd3f9c7e427e47f3203c64ea91c27bcc444eebd74cd"),
    "sdl3-devel": (f"https://github.com/libsdl-org/SDL/releases/download/release-{SDL_VERSION}/"
                   f"SDL3-devel-{SDL_VERSION}-mingw.tar.gz",
                   "d2d3bf3fb97a90b22e908a306d7362631ef1bab229d988fe6669595dc29cd9ed"),
    "ninja": ("https://github.com/ninja-build/ninja/releases/download/v1.13.2/ninja-win.zip",
              "07fc8261b42b20e71d1720b39068c2e14ffcee6396b76fb7a795fb460b78dc65"),
    "opus": (MSYS2 + "mingw-w64-ucrt-x86_64-opus-1.6.1-2-any.pkg.tar.zst",
             "bb114bcf26225f50f874497ebfd6b8224fff2a20bda3f8ee45cd07ce5f02657b"),
    "libvpx": (MSYS2 + "mingw-w64-ucrt-x86_64-libvpx-1.17.0-1-any.pkg.tar.zst",
               "8ff6919dbb88ea26ce28d22fd81d6f0d6e200cf56a4dd81ffc2d33124fe98024"),
    "vulkan-headers": (MSYS2 + "mingw-w64-ucrt-x86_64-vulkan-headers-1~1.4.363.0-1-any.pkg.tar.zst",
                       "be602b2657cace531316e78b1eda4087e8f7f84ebb877f08fba198a2db06406a"),
    "vulkan-loader": (MSYS2 + "mingw-w64-ucrt-x86_64-vulkan-loader-1~1.4.363.0-1-any.pkg.tar.zst",
                      "065076fe13d39ab97eea4740596426d5ae741b5044e48e1c9495d04835c92dbe"),
    "vma": (MSYS2 + "mingw-w64-ucrt-x86_64-vulkan-memory-allocator-3.4.0-1-any.pkg.tar.zst",
            "44c81162337915afecda3185f66fd98268a54454580b2717080c9dfef57fe781"),
    "zstd": (MSYS2 + "mingw-w64-ucrt-x86_64-zstd-1.5.7-2-any.pkg.tar.zst",
             "dbdb8427280046a2b41697780aa4c52983b708082b0da4755951dc3bea96ca89"),
    "zlib": (MSYS2 + "mingw-w64-ucrt-x86_64-zlib-1.4-2-any.pkg.tar.zst",
             "172541cf5e2391a402f092104988699b0163129d22be1d6b4b201bfb84af6fcf"),
    "libusb": (MSYS2 + "mingw-w64-ucrt-x86_64-libusb-1.0.30-1-any.pkg.tar.zst",
               "20106e6ff31b4581bb111b28a68b6b79a50dc3e5fc3033944e1c4b3ff8cd812e"),
}
# The Python packages the engine's generator and NUP tools need (upstream tools/requirements-hwaoi.txt).
PY_PACKAGES = ["numpy==2.5.4", "lz4==4.4.5", "zstandard==0.25.0", "capstone==5.0.7"]
# llvm-mingw: only the x86_64 Windows target is needed.
LLVM_SKIP_TOP = {"aarch64-w64-mingw32", "armv7-w64-mingw32", "i686-w64-mingw32", "busybox", "python", "share"}
LINUX_PACKAGES = ["clang", "lld", "ninja-build", "libsdl3-dev", "libopus-dev", "libvpx-dev", "libzstd-dev",
                  "libvulkan-dev", "libvulkan-memory-allocator-dev", "zlib1g-dev", "python3-venv", "dotnet-sdk-10.0", "git"]


# ── small helpers ────────────────────────────────────────────────────────────────────────────────────────────────────

def log(msg: str) -> None:
    print(f"[engine] {msg}", flush=True)


def die(msg: str) -> None:
    print(f"[engine] ERROR: {msg}", file=sys.stderr, flush=True)
    raise SystemExit(1)


def run(cmd, cwd=None, env=None, check=True, capture=False):
    cmd = [str(c) for c in cmd]
    log("$ " + subprocess.list2cmdline(cmd))
    r = subprocess.run(cmd, cwd=cwd, env=env, text=True, capture_output=capture)
    if check and r.returncode:
        if capture:
            sys.stderr.write((r.stdout or "") + (r.stderr or ""))
        die(f"command failed with exit {r.returncode}: {cmd[0]}")
    return r


def git(*args, cwd=ENGINE, capture=True, check=True):
    return run(["git", *args], cwd=cwd, capture=capture, check=check)


def sha256(path: Path) -> str:
    with open(path, "rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def fetch(name: str) -> Path:
    """Pinned download into .cache/downloads (resumable), verified against its SHA-256."""
    url, want = PINS[name]
    DL.mkdir(parents=True, exist_ok=True)
    path = DL / url.rsplit("/", 1)[1].replace("%7E", "~")
    if path.is_file() and sha256(path) == want:
        return path
    if path.is_file():
        log(f"{path.name}: not the pinned file, downloading again")
        path.unlink()
    part = path.with_name(path.name + ".part")
    have = part.stat().st_size if part.is_file() else 0
    log(f"download {url}" + (f" (resuming at {have} bytes)" if have else ""))
    req = urllib.request.Request(url, headers={"User-Agent": "age-of-decomp-engine-tool",
                                               **({"Range": f"bytes={have}-"} if have else {})})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            if have and r.status != 206:
                have = 0
            with open(part, "ab" if have else "wb") as f:
                shutil.copyfileobj(r, f, 1 << 20)
    except urllib.error.HTTPError as e:
        if e.code != 416:
            raise
    part.replace(path)
    got = sha256(path)
    if got != want:
        path.unlink()
        die(f"{path.name}: sha256 {got}, pinned {want}; deleted, run again")
    return path


def unpack(name: str) -> Path:
    """Extract a pinned archive once into .cache/unpacked/<name> and return that folder."""
    dest = UNPACK / name
    stamp = dest / ".unpacked"
    if stamp.is_file() and stamp.read_text() == PINS[name][1]:
        return dest
    shutil.rmtree(dest, ignore_errors=True)
    dest.mkdir(parents=True)
    path = fetch(name)
    log(f"unpack {path.name}")
    if path.suffix in (".zip", ".nupkg"):
        with zipfile.ZipFile(path) as z:
            z.extractall(dest)
    else:  # .tar.gz / .tar.zst (Python 3.14 reads zstd natively)
        with tarfile.open(path, "r:*") as t:
            t.extractall(dest, filter="data")
    stamp.write_text(PINS[name][1])
    return dest


def copy_tree(src: Path, dst: Path, skip=lambda rel: False) -> int:
    n = 0
    for p in sorted(src.rglob("*")):
        rel = p.relative_to(src).as_posix()
        if p.is_dir() or skip(rel):
            continue
        out = dst / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, out)
        n += 1
    return n


def upstream() -> dict:
    return json.loads(UPSTREAM.read_text(encoding="utf-8"))


# ── toolchain ────────────────────────────────────────────────────────────────────────────────────────────────────────

def toolchain_windows() -> None:
    TC.mkdir(parents=True, exist_ok=True)
    # llvm-mingw (clang, lld, libc++, mingw-w64 headers/libs) for x86_64 only
    llvm = TC / "llvm"
    if not (llvm / "bin" / "clang.exe").is_file():
        src = unpack("llvm-mingw") / f"llvm-mingw-{LLVM_VERSION}-ucrt-x86_64"
        n = copy_tree(src, llvm, lambda rel: rel.split("/")[0] in LLVM_SKIP_TOP or rel.startswith("include/ddk/"))
        log(f"llvm-mingw {LLVM_VERSION}: {n} files")
    # ninja + zlib1.dll (the runtime loads zlib dynamically for native inflate: HWAOI_ZLIB_DLL)
    (TC / "bin").mkdir(exist_ok=True)
    shutil.copy2(unpack("ninja") / "ninja.exe", TC / "bin" / "ninja.exe")
    shutil.copy2(unpack("zlib") / "ucrt64" / "bin" / "zlib1.dll", TC / "bin" / "zlib1.dll")
    # deps/ = include/, lib/, bin/ as tools/build_game.py --deps expects
    deps = TC / "deps"
    for sub in ("include", "lib", "bin"):
        (deps / sub).mkdir(parents=True, exist_ok=True)
    sdl = unpack("sdl3-devel") / f"SDL3-{SDL_VERSION}" / "x86_64-w64-mingw32"
    copy_tree(sdl / "include", deps / "include")
    shutil.copy2(sdl / "lib" / "libSDL3.dll.a", deps / "lib")
    shutil.copy2(sdl / "bin" / "SDL3.dll", deps / "bin")
    for pkg, incs, libs, dlls in (("opus", ["opus"], ["libopus.dll.a"], ["libopus-0.dll"]),
                                  ("libvpx", ["vpx"], ["libvpx.a"], []),
                                  ("vulkan-headers", ["vulkan", "vk_video"], [], []),
                                  ("vulkan-loader", [], ["libvulkan-1.dll.a"], []),
                                  ("vma", ["vma"], [], [])):
        root = unpack(pkg) / "ucrt64"
        for inc in incs:
            copy_tree(root / "include" / inc, deps / "include" / inc)
        for lib in libs:
            shutil.copy2(root / "lib" / lib, deps / "lib")
        for dll in dlls:
            shutil.copy2(root / "bin" / dll, deps / "bin")
    zstd = unpack("zstd") / "ucrt64"
    for name in ("zstd.h", "zstd_errors.h"):
        shutil.copy2(zstd / "include" / name, deps / "include")
    shutil.copy2(zstd / "lib" / "libzstd.a", deps / "lib")
    # licences of what we redistribute
    lic = TC / "licenses"
    lic.mkdir(exist_ok=True)
    for pkg, rel in (("opus", "opus/COPYING"), ("libvpx", "libvpx/LICENSE"), ("vulkan-headers", "vulkan-headers/LICENSE"),
                     ("vulkan-loader", "vulkan-loader/LICENSE"), ("vma", "vulkan-memory-allocator/LICENSE.txt"),
                     ("zstd", "zstd/LICENSE"), ("zlib", "zlib/LICENSE")):
        p = unpack(pkg) / "ucrt64" / "share" / "licenses" / rel
        if p.is_file():
            shutil.copy2(p, lic / f"{pkg}-{p.name}.txt")
    sdl_lic = unpack("sdl3-devel") / f"SDL3-{SDL_VERSION}" / "LICENSE.txt"
    if sdl_lic.is_file():
        shutil.copy2(sdl_lic, lic / "SDL3-LICENSE.txt")
    shutil.copy2(llvm / "LICENSE.TXT", lic / "llvm-mingw-LICENSE.txt")
    # Python: the official NuGet layout (full stdlib, no ._pth) + the generator's packages
    py = TC / "python"
    if not (py / "python.exe").is_file():
        copy_tree(unpack("python") / "tools", py)
    have = subprocess.run([str(py / "python.exe"), "-I", "-c", "import numpy, lz4.block, zstandard, capstone"],
                          capture_output=True)
    if have.returncode:
        site = py / "Lib" / "site-packages"
        uv = shutil.which("uv")
        if uv:
            run([uv, "pip", "install", "--target", site, "--python", py / "python.exe", "--only-binary", ":all:",
                 *PY_PACKAGES], env=dict(os.environ, UV_CACHE_DIR=str(CACHE / "uv-cache")))
        else:
            run([py / "python.exe", "-m", "ensurepip"], check=False)
            run([py / "python.exe", "-m", "pip", "install", "--only-binary", ":all:", "--target", site, *PY_PACKAGES])
        shutil.rmtree(site / "bin", ignore_errors=True)
    check = run([py / "python.exe", "-I", "-c", "import sys, numpy, lz4.block, zstandard, capstone; "
                 "print('python', sys.version.split()[0], 'numpy', numpy.__version__)"], capture=True)
    log(check.stdout.strip())
    # .NET SDK: a junction to the system SDK, so the translator's NuGet cache lands in .cache (upstream puts it next
    # to the dotnet folder, which is not writable under Program Files).
    dn = TC / "DotNet"
    if not (dn / "dotnet.exe").is_file():
        system = shutil.which("dotnet")
        if not system:
            die(".NET 10 SDK not found: install it (winget install Microsoft.DotNet.SDK.10) and run again")
        target = Path(system).resolve().parent
        run(["cmd", "/c", "mklink", "/J", str(dn), str(target)])
    log(f"toolchain ready in {TC}")


def toolchain_linux() -> None:
    missing = [p for p in ("clang", "ld.lld", "ninja", "dotnet", "git") if not shutil.which(p)]
    if missing:
        die("missing tools: " + ", ".join(missing) + "\n  Ubuntu/Debian: sudo apt install " + " ".join(LINUX_PACKAGES))
    venv = TC / "venv"
    if not (venv / "bin" / "python").is_file():
        run([sys.executable, "-m", "venv", venv])
    run([venv / "bin" / "python", "-m", "pip", "install", "--only-binary", ":all:", *PY_PACKAGES])
    log(f"toolchain ready in {TC} (system clang/ninja/SDL3/Vulkan; Python venv)")


def cmd_toolchain(_args) -> None:
    toolchain_windows() if WIN else toolchain_linux()


# ── engine checkout and patch queue ──────────────────────────────────────────────────────────────────────────────────

def cmd_fetch(args) -> None:
    up = upstream()
    if not ENGINE.is_dir():
        ENGINE.parent.mkdir(parents=True, exist_ok=True)
        ref = ROOT / "refs" / "hwaoi-recomp"
        if ref.is_dir():  # fast local clone from the read-only reference, then point origin at upstream
            run(["git", "clone", "--no-hardlinks", str(ref), str(ENGINE)])
            git("remote", "set-url", "origin", up["repository"])
        else:
            run(["git", "clone", "--filter=blob:none", up["repository"], str(ENGINE)])
        git("config", "user.name", "Age of Decomp patch queue")
        git("config", "user.email", "aod-patches@localhost")
        git("config", "core.autocrlf", "false")
    if git("cat-file", "-e", up["commit"] + "^{commit}", check=False).returncode:
        git("fetch", "origin", capture=False)
    status = git("status", "--porcelain").stdout.strip()
    if status and not args.force:
        die(f"{ENGINE} has uncommitted changes; commit them on branch aod (then export-patches) or pass --force")
    current = git("rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    if current == "aod" and not args.force:
        base = git("merge-base", "HEAD", up["commit"]).stdout.strip()
        if base == git("rev-parse", up["commit"]).stdout.strip():
            log("branch aod already on the pin; use --force to rebuild it from engine/patches")
            return
    git("checkout", "-B", "aod", up["commit"], "--force", capture=False)
    git("clean", "-fdq", "-e", "work/")
    patches = sorted(PATCHES.glob("*.patch"))
    for p in patches:
        r = git("am", "--3way", "--keep-cr", str(p), check=False)
        if r.returncode:
            git("am", "--abort", check=False)
            die(f"{p.name} does not apply on {up['commit'][:10]}: rebase it by hand in {ENGINE}")
    log(f"engine at {up['commit'][:10]} + {len(patches)} patch(es) on branch aod")


def cmd_export_patches(_args) -> None:
    up = upstream()
    if git("status", "--porcelain").stdout.strip():
        die("commit your engine changes on branch aod first")
    PATCHES.mkdir(parents=True, exist_ok=True)
    for old in PATCHES.glob("*.patch"):
        old.unlink()
    git("format-patch", "--zero-commit", "--no-signature", "--full-index", "-N", "-o", str(PATCHES),
        f"{up['commit']}..aod")
    log(f"{len(list(PATCHES.glob('*.patch')))} patch(es) in {PATCHES.relative_to(ROOT)}")


# ── shader translator ────────────────────────────────────────────────────────────────────────────────────────────────

def cmd_translator(args) -> None:
    if not ENGINE.is_dir():
        die("run `engine.py fetch` first")
    dotnet = TC / "DotNet" / "dotnet.exe" if WIN else Path(shutil.which("dotnet") or "dotnet")
    python = TC / "python" / "python.exe" if WIN else TC / "venv" / "bin" / "python"
    env = dict(os.environ, PYTHONUTF8="1", HWAOI_NUGET_CACHE=str(TC / "NuGet"))  # engine patch 0001
    env.pop("HWAOI_TOOLS", None)
    if XLATE_OUT.exists():
        shutil.rmtree(XLATE_OUT)  # the builder refuses to overwrite a deployment
    run([python, "-E", "-s", ENGINE / "tools" / "build_xlate_hwaoi.py", "--dotnet", dotnet, "--out", XLATE_OUT,
         "--online", "--no-vendor-reference"], cwd=ENGINE, env=env)
    log(f"translator in {XLATE_OUT}")


# ── staging the portable bundle ──────────────────────────────────────────────────────────────────────────────────────

def source_files():
    raw = git("ls-files", "-z").stdout
    for name in sorted(set(raw.split("\0")) - {""}):
        top = name.split("/", 1)[0]
        if top in ("compiler", "config", "recomp", "runtime", "tools", "tests") or name in (
                "LICENSE", "LICENSE-MIT", "THIRD_PARTY_NOTICES.md", "README.md", "VERSION", "AGENTS.md"):
            yield name


def cmd_stage(args) -> None:
    if not WIN:
        die("Linux staging is done by tools/release.py --linux (AppImage); not implemented yet (TASKS X-3)")
    for need in (TC / "llvm" / "bin" / "clang.exe", TC / "python" / "python.exe", XLATE_OUT / "totk_xlate.exe"):
        if not need.is_file():
            die(f"missing {need}: run toolchain / translator first")
    if STAGE.exists():
        shutil.rmtree(STAGE)
    STAGE.mkdir(parents=True)
    n = 0
    for rel in source_files():
        dst = STAGE / "source" / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ENGINE / rel, dst)
        n += 1
    log(f"source: {n} files")
    copy_tree(TC / "python", STAGE / "python", lambda rel: "__pycache__" in rel or rel.endswith(".pyc"))
    copy_tree(TC / "llvm", STAGE / "llvm")
    copy_tree(TC / "deps", STAGE / "deps")
    copy_tree(XLATE_OUT, STAGE / "translator", lambda rel: rel.endswith("build-report.json") or rel.endswith(".pdb"))
    (STAGE / "bin").mkdir()
    shutil.copy2(TC / "bin" / "ninja.exe", STAGE / "bin" / "ninja.exe")
    shutil.copy2(TC / "bin" / "zlib1.dll", STAGE / "bin" / "zlib1.dll")
    copy_tree(TC / "licenses", STAGE / "licenses")
    revision = git("rev-parse", "HEAD").stdout.strip()
    files = {p.relative_to(STAGE).as_posix(): sha256(p) for p in sorted(STAGE.rglob("*")) if p.is_file()}
    version = (ROOT / "VERSION").read_text().strip() if (ROOT / "VERSION").is_file() else "0.0.0-dev"
    manifest = {"format": "hwaoi-compiler-v1", "version": version, "source_revision": revision,
                "aod_upstream": upstream()["commit"], "files": files, "game_content_included": False}
    (STAGE / "bundle.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    size = sum((STAGE / p).stat().st_size for p in files)
    log(f"staged {len(files)} files, {size / 2**20:.0f} MiB in {STAGE}")
    run([STAGE / "python" / "python.exe", "-E", "-s", STAGE / "source" / "compiler" / "backend.py", "check",
         "--bundle", STAGE], cwd=STAGE)


# ── developer game build ─────────────────────────────────────────────────────────────────────────────────────────────

def cmd_game(args) -> None:
    nup = Path(args.nup).resolve(strict=True)
    out = Path(args.out).resolve()
    python = STAGE / "python" / "python.exe"
    if not python.is_file():
        die("run `engine.py stage` first")
    cmd = [python, "-E", "-s", STAGE / "source" / "compiler" / "backend.py", "build", "--bundle", STAGE,
           "--nup", nup, "--output", out, "--jobs", str(args.jobs), "--resolution", args.resolution]
    started = time.time()
    run(cmd, cwd=STAGE)
    log(f"game built in {(time.time() - started) / 60:.1f} min -> {out}")


def cmd_status(_args) -> None:
    up = upstream()
    rows = [("pin", f"{up['repository']} @ {up['commit'][:12]}"),
            ("toolchain", "ok" if (TC / ("llvm/bin/clang.exe" if WIN else "venv/bin/python")).exists() else "missing"),
            ("engine checkout", git("rev-parse", "--short", "HEAD").stdout.strip() if ENGINE.is_dir() else "missing"),
            ("patches", str(len(list(PATCHES.glob("*.patch"))))),
            ("translator", "ok" if (XLATE_OUT / ("totk_xlate.exe" if WIN else "totk_xlate")).is_file() else "missing"),
            ("bundle", "ok" if (STAGE / "bundle.json").is_file() else "missing")]
    for k, v in rows:
        print(f"{k:16s} {v}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("toolchain")
    f = sub.add_parser("fetch")
    f.add_argument("--force", action="store_true")
    sub.add_parser("export-patches")
    sub.add_parser("translator")
    sub.add_parser("stage")
    g = sub.add_parser("game")
    g.add_argument("--nup", required=True)
    g.add_argument("--out", default=str(ROOT / "work" / "game"))
    g.add_argument("--jobs", type=int, default=0)
    g.add_argument("--resolution", default="1920x1080", choices=("1280x720", "1920x1080", "2560x1440", "3840x2160"))
    sub.add_parser("status")
    args = ap.parse_args()
    {"toolchain": cmd_toolchain, "fetch": cmd_fetch, "export-patches": cmd_export_patches,
     "translator": cmd_translator, "stage": cmd_stage, "game": cmd_game, "status": cmd_status}[args.cmd](args)


if __name__ == "__main__":
    main()
