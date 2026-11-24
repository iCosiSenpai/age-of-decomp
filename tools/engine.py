#!/usr/bin/env python3
"""Age of Decomp engine tooling (Python 3.12+, standard library only; Windows x64 and Linux x64).

  py tools/engine.py toolchain         pinned tools and libraries  -> .cache/toolchain/<os>/
  py tools/engine.py translator        Ryujinx-based shader translator (self-contained .NET publish)
  py tools/engine.py stage             portable engine bundle -> out/engine/<os>/ (format hwaoi-compiler-v1)
  py tools/engine.py game --nup F [--out D]   developer build of the game from a NUP with the staged bundle
  py tools/engine.py status            what is present

Every download is pinned by URL and SHA-256 (pins shared with iCosiSenpai/totk-pc-recomp tools/make_release.py, MIT).
Nothing is installed system-wide. No game content is ever written outside ignored folders.

Linux: the bundle carries its whole compiler too, so a player's PC (a Steam Deck too) needs no development packages:
LLVM's official clang and lld, the files of the Steam Runtime 3 "sniper" SDK the build reads (glibc 2.31, GCC 14's
libstdc++), SDL3, Opus, libvpx and the Vulkan loader's link library from the same SDK, zstd built static, Ninja, and a
pinned relocatable CPython (python-build-standalone) with the generator's packages. Only the developer steps need the
system's .NET 10 SDK and git (`translator`), make and tar (`toolchain`).
The engine's sources are in this repository, engine/source (engine/ORIGIN.md says where they came from).
AOD_CACHE_DIR moves .cache elsewhere, e.g. a toolchain of its own for WSL when the folder is shared with Windows.
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
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WIN = os.name == "nt"
OSNAME = "windows" if WIN else "linux"
CACHE = Path(os.environ.get("AOD_CACHE_DIR") or ROOT / ".cache").expanduser().resolve()
DL = CACHE / "downloads"
TC = CACHE / "toolchain" / OSNAME
UNPACK = CACHE / "unpacked"
ENGINE = ROOT / "engine" / "source"
STAGE = ROOT / "out" / "engine" / OSNAME
# build_xlate_hwaoi.py requires this parent (engine/source/work is ignored); one folder per OS, so a checkout shared
# by Windows and WSL keeps both
XLATE_OUT = ENGINE / "work" / "build" / "hwaoi196608" / ("xlate-managed-irbanks" if WIN else "xlate-linux-x64")
XLATE_EXE = "totk_xlate.exe" if WIN else "totk_xlate"

MSYS2 = "https://repo.msys2.org/mingw/ucrt64/"
SDL_VERSION = "3.4.20"
LLVM_VERSION = "20261020"
PY_VERSION = "3.14.9"
# Linux: LLVM's own release and the Steam Runtime 3 "sniper" SDK snapshot (toolchain_linux)
LINUX_LLVM = "23.1.4"
SNIPER = "3.0.20261021.266025"
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
    # SDL source: the official Windows DLL has no Switch 2 driver (SDL compiles it only with libusb, HAVE_LIBUSB)
    "sdl3-src": (f"https://github.com/libsdl-org/SDL/releases/download/release-{SDL_VERSION}/SDL3-{SDL_VERSION}.tar.gz",
                 "f3bb06f55bed9557227a700af2ea62e49497ebdf58037732a731a5fb3ef8581f"),
    # Linux: relocatable CPython (astral-sh/python-build-standalone, stripped install_only build)
    "python-linux": ("https://github.com/astral-sh/python-build-standalone/releases/download/20261110/"
                     "cpython-3.14.10%2B20261110-x86_64-unknown-linux-gnu-install_only_stripped.tar.gz",
                     "c2ff2e97422f5d3896a16e0317fd212ba977bbf2688d93a6f7a03d817506bd30"),
    # Linux release packaging (tools/release.py linux --appimage): the AppImage type 2 runtime the squashfs follows
    "appimage-runtime": ("https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64",
                         "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"),
    # Linux toolchain (toolchain_linux): the pins of DKB Recomp's tools/make_release_linux.py. LLVM's binary release
    # carries no licence text, hence its LICENSE.TXT from the same tag (the third field names files that would clash).
    "llvm-linux": (f"https://github.com/llvm/llvm-project/releases/download/llvmorg-{LINUX_LLVM}/"
                   f"LLVM-{LINUX_LLVM}-Linux-X64.tar.xz",
                   "25d814fda50d5fb0084b42be09605d8b1595d49d5e2e03a3345d7e112ff6f9f3"),
    "llvm-license": (f"https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-{LINUX_LLVM}/llvm/LICENSE.TXT",
                     "8d85c1057d742e597985c7d4e6320b015a9139385cff4cbae06ffc0ebe89afee", f"llvm-{LINUX_LLVM}-LICENSE.TXT"),
    "sniper-sdk": ("https://repo.steampowered.com/steamrt-images-sniper/snapshots/"
                   f"{SNIPER}/com.valvesoftware.SteamRuntime.Sdk-amd64%2Ci386-sniper-sysroot.tar.gz",
                   "8f71947035f97925a311b2ed753f22efcf2f13d6f1631945b8e537f8903ce2cf"),
    "ninja-linux": ("https://github.com/ninja-build/ninja/releases/download/v1.13.2/ninja-linux.zip",
                    "5749cbc4e668273514150a80e387a957f933c6ed3f5f11e03fb30955e2bbead6"),
    "zstd-src": ("https://github.com/facebook/zstd/releases/download/v1.5.7/zstd-1.5.7.tar.gz",
                 "eb33e51f49a15e023950cd7825ca74a4a2b43db8354825ac24fc1b7ee09e6fa3"),
    "vulkan-headers-src": ("https://github.com/KhronosGroup/Vulkan-Headers/archive/refs/tags/v1.4.363.tar.gz",
                           "cbaf687d3c59b9666fe080e7f8396b6b4f4d344a768ee6b337c59852f4526b68",
                           "Vulkan-Headers-1.4.363.tar.gz"),
    "vma-header": ("https://raw.githubusercontent.com/GPUOpen-LibrariesAndSDKs/VulkanMemoryAllocator/v3.4.0/include/"
                   "vk_mem_alloc.h", "8487b7995ad3b263eb73bc5b9a77d71aa69b6bef5d58a715c02d2663afd81f1a"),
    # NVIDIA Streamline (MIT) for DLAA on Windows: only the three signed production DLLs below and license.txt are
    # taken from it. The SDK also carries nvngx_dlss*.dll (NVIDIA RTX SDK licence): never staged or shipped.
    "streamline-sdk": ("https://github.com/NVIDIA-RTX/Streamline/releases/download/v2.14.16/streamline-sdk-v2.14.16.zip",
                       "32041e51db0cab709a271b72a352281b690c2abbd669a827809401a5dd23956b"),
}
# Streamline's own modules (MIT, the SDK's license.txt; signed by NVIDIA): DLSS, and for frame generation DLSS-G, Reflex
# and its latency marks. Never the SDK's other binaries: nvngx_*.dll (NVIDIA RTX SDK licence) and NvLowLatencyVk.dll
# (NVIDIA's SDK licence, bin/x64/reflex.license.txt; Reflex runs through the driver's VK_NV_low_latency2 instead).
STREAMLINE_DLLS = ("sl.interposer.dll", "sl.common.dll", "sl.dlss.dll", "sl.dlss_g.dll", "sl.reflex.dll", "sl.pcl.dll")
# The licence texts the Windows toolchain gathers for what the bundle redistributes (programs, DLLs, static libraries
# and headers in its bin/, deps/ and llvm/). `toolchain` and `stage` both stop when one is missing.
WINDOWS_LICENSES = ("SDL3-LICENSE.txt", "libusb-COPYING.txt", "llvm-mingw-LICENSE.txt", "opus-COPYING.txt",
                    "vulkan-headers-LICENSE.txt", "vulkan-loader-LICENSE.txt", "vma-LICENSE.txt", "zlib-LICENSE.txt",
                    "zstd-LICENSE.txt", "libvpx-LICENSE.txt", "libvpx-PATENTS.txt", "ninja-LICENSE.txt")
# The Python packages the engine's generator and NUP tools need (upstream tools/requirements-hwaoi.txt).
PY_REQUIREMENTS = ROOT / "tools/requirements-engine.txt"
# llvm-mingw: only the x86_64 Windows target is needed.
LLVM_SKIP_TOP = {"aarch64-w64-mingw32", "armv7-w64-mingw32", "i686-w64-mingw32", "busybox", "python", "share"}
# Linux: the release toolchain (toolchain_linux), the model of DKB Recomp's tools/make_release_linux.py
# (Lucas-blip-png/dkb-pc-recomp, GPL-3.0-or-later), ported by hand. LLVM's official binaries (they need glibc 2.34)
# compile and link against the files of the sniper SDK the build reads, taken by package from the SDK's own dpkg lists,
# so the game runs on SteamOS and on any distribution at least as new as Debian 11 (glibc 2.31). The SDK keeps GCC 14
# under /usr/lib/gcc-14: its runtime and libstdc++ go where clang looks for a GCC installation in a sysroot (the runtime
# needs C++20's library; the SDK's default GCC 10 is left out).
SYSROOT_PACKAGES = ("libc6", "libc6-dev", "linux-libc-dev", "libgcc-s1", "libgcc-14-dev", "libstdc++-14-dev")
# The libraries the game links, from the same SDK, into deps/: Opus and libvpx static and the Vulkan loader's link
# library (the PC's libvulkan.so.1 runs). zstd is built static; SDL 3.4.20 (the Windows bundle's version) is built in
# the SDK's own image, so its X11, Wayland, audio, libdecor and libusb parts load only where the PC has them (the SDK's
# libSDL3 needs all of them to start); libSDL3.so.0 goes beside the game (rpath $ORIGIN). The Vulkan headers and VMA
# are the Windows bundle's versions.
DEPS_PACKAGES = ("libopus-dev", "libvpx-dev", "libvulkan1", "libvulkan-dev")
GCC14_MOVES = (("usr/lib/gcc-14/lib/gcc/x86_64-linux-gnu/14/", "usr/lib/gcc/x86_64-linux-gnu/14/"),
               ("usr/lib/gcc-14/x86_64-linux-gnu/include/c++/14/", "usr/include/c++/14/"))
# glibc's libraries a link reads (lib<name>.so.N and the scripts that name them), and the dynamic loader
GLIBC_LIBS = ("libc", "libm", "libmvec", "libdl", "libpthread", "librt", "libutil")
# The licence texts the Linux toolchain gathers for what the bundle redistributes (llvm/, deps/, bin/): `toolchain`
# and `stage` both stop when one is missing.
LINUX_LICENSES = ("llvm-LICENSE.txt", "glibc-copyright.txt", "linux-libc-dev-copyright.txt", "gcc-14-copyright.txt",
                  "SDL3-LICENSE.txt", "opus-copyright.txt", "libvpx-copyright.txt", "libvpx-PATENTS.txt",
                  "vulkan-loader-copyright.txt", "vulkan-headers-LICENSE.txt", "vma-LICENSE.txt", "zstd-LICENSE.txt",
                  "ninja-LICENSE.txt", "python-LICENSE.txt")
# Developer tools the Linux steps run (the player's PC needs none of them)
LINUX_DEV_TOOLS = {"dotnet": "translator (.NET 10 SDK)", "git": "translator"}


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
    url, want, *named = PINS[name]
    DL.mkdir(parents=True, exist_ok=True)
    path = DL / (named[0] if named else urllib.parse.unquote(url.rsplit("/", 1)[1]))
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
        if path.name.endswith(".zst") and sys.version_info < (3, 14):
            die(f"{path.name}: Python 3.14 or newer reads .tar.zst; this is {sys.version.split()[0]}")
        with tarfile.open(path, "r:*") as t:
            t.extractall(dest, filter="data")
    stamp.write_text(PINS[name][1])
    return dest


def installed(stamp: Path, key: str) -> bool:
    """Whether the part `stamp` belongs to was installed for `key` (its pins); toolchain parts are made again when a
    pin changes, not only when a file is missing."""
    return stamp.is_file() and stamp.read_text(encoding="utf-8") == key


def python_packages(exe: Path, stamp: Path, install) -> None:
    """The generator's packages into the toolchain's Python, again whenever requirements-engine.txt changes (a pin
    moved): `install()` puts them there; the stamp then records the requirements it installed."""
    key = sha256(PY_REQUIREMENTS)
    ok = subprocess.run([exe, "-I", "-c", "import numpy, lz4.block, zstandard, capstone"], capture_output=True)
    if ok.returncode or not installed(stamp, key):
        install()
        stamp.write_text(key, encoding="utf-8")


def copy_tree(src: Path, dst: Path, skip=lambda rel: False) -> int:
    """Copy the files of src (symlinks stay symlinks, as in python-build-standalone's bin/)."""
    n = 0
    for p in sorted(src.rglob("*")):
        rel = p.relative_to(src).as_posix()
        if skip(rel) or p.is_dir() and not p.is_symlink():
            continue
        out = dst / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        if p.is_symlink():
            if out.is_symlink() or out.exists():
                out.unlink()
            out.symlink_to(os.readlink(p))
        else:
            shutil.copy2(p, out)
        n += 1
    return n


def toolchain_windows() -> None:
    TC.mkdir(parents=True, exist_ok=True)
    # llvm-mingw (clang, lld, libc++, mingw-w64 headers/libs) for x86_64 only
    llvm = TC / "llvm"
    llvm_stamp = TC / "llvm-mingw.installed"
    if not (llvm / "bin" / "clang.exe").is_file() or not installed(llvm_stamp, PINS["llvm-mingw"][1]):
        shutil.rmtree(llvm, ignore_errors=True)
        src = unpack("llvm-mingw") / f"llvm-mingw-{LLVM_VERSION}-ucrt-x86_64"
        n = copy_tree(src, llvm, lambda rel: rel.split("/")[0] in LLVM_SKIP_TOP or rel.startswith("include/ddk/"))
        llvm_stamp.write_text(PINS["llvm-mingw"][1], encoding="utf-8")
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
    if not installed(TC / "sdl3-libusb.installed", sdl_libusb_key()):
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
    for pkg, rel in (("opus", "opus/COPYING"), ("vulkan-headers", "vulkan-headers/LICENSE"),
                     ("vulkan-loader", "vulkan-loader/LICENSE"), ("vma", "VulkanMemoryAllocator/LICENSE"),
                     ("zstd", "zstd/LICENSE"), ("zlib", "zlib/LICENSE")):
        p = unpack(pkg) / "ucrt64" / "share" / "licenses" / rel
        if not p.is_file():   # never skipped in silence: the bundle would ship the part without its licence
            die(f"the {pkg} package has no {rel} under share/licenses: find its licence file and name it here")
        shutil.copy2(p, lic / f"{pkg}-{p.name}.txt")
    # Parts whose package carries no licence file (the MSYS2 libvpx package, Ninja's zip): the texts kept in the
    # repository, libvpx's own LICENSE and PATENTS and the Apache License 2.0 Ninja is under.
    for p in sorted((ROOT / "packaging" / "licenses").glob("*.txt")):
        shutil.copy2(p, lic / p.name)
    sdl_lic = unpack("sdl3-devel") / f"SDL3-{SDL_VERSION}" / "LICENSE.txt"
    if not sdl_lic.is_file():
        die(f"the SDL package has no LICENSE.txt at {sdl_lic}")
    shutil.copy2(sdl_lic, lic / "SDL3-LICENSE.txt")
    shutil.copy2(llvm / "LICENSE.TXT", lic / "llvm-mingw-LICENSE.txt")
    build_sdl_libusb(llvm, deps, lic)
    missing = [name for name in WINDOWS_LICENSES if not (lic / name).is_file()]
    if missing:
        die(f"licence files missing from {lic}: {', '.join(missing)}")
    # Python: the official NuGet layout (full stdlib, no ._pth) + the generator's packages
    py = TC / "python"
    py_stamp = TC / "python.installed"
    if not (py / "python.exe").is_file() or not installed(py_stamp, PINS["python"][1]):
        shutil.rmtree(py, ignore_errors=True)
        copy_tree(unpack("python") / "tools", py)
        py_stamp.write_text(PINS["python"][1], encoding="utf-8")

    def install_windows_packages() -> None:
        site = py / "Lib" / "site-packages"
        uv = shutil.which("uv")
        if uv:
            run([uv, "pip", "install", "--target", site, "--python", py / "python.exe", "--only-binary", ":all:",
                 "--require-hashes", "-r", PY_REQUIREMENTS], env=dict(os.environ, UV_CACHE_DIR=str(CACHE / "uv-cache")))
        else:
            run([py / "python.exe", "-m", "ensurepip"], check=False)
            run([py / "python.exe", "-m", "pip", "install", "--only-binary", ":all:", "--require-hashes",
                 "--target", site, "-r", PY_REQUIREMENTS])
        shutil.rmtree(site / "bin", ignore_errors=True)
    python_packages(py / "python.exe", TC / "python-packages.installed", install_windows_packages)
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


SWITCH2_MARKERS = (b"SDL_JOYSTICK_HIDAPI_SWITCH2", b"libusb_init")


def sdl_has_switch2(dll: Path) -> bool:
    """The SDL3.dll carries the Switch 2 HIDAPI driver and its libusb loader (the check NxToWin uses)."""
    data = dll.read_bytes() if dll.is_file() else b""
    return all(m in data for m in SWITCH2_MARKERS)


def find_cmake() -> Path | None:
    found = shutil.which("cmake")
    if found:
        return Path(found)
    import glob
    hits = glob.glob(r"C:\Program Files\Microsoft Visual Studio\*\*\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe")
    return Path(sorted(hits)[-1]) if hits else None


def sdl_libusb_key() -> str:
    """What our own SDL3.dll is built from: SDL's source, libusb and the compiler."""
    return "+".join(PINS[name][1] for name in ("sdl3-src", "libusb", "llvm-mingw"))


def build_sdl_libusb(llvm: Path, deps: Path, lic: Path) -> None:
    """SDL 3 from source with HIDAPI over libusb, loaded at run time (`libusb-1.0.dll` beside SDL3.dll). It replaces the
    official DLL, which lacks the Switch 2 driver: wired Switch 2 Pro Controller / Joy-Con 2 / GameCube pads need it on
    Windows. Same version and headers as the official package; only the HIDAPI libusb backend is added."""
    dll = deps / "bin" / "SDL3.dll"
    stamp = TC / "sdl3-libusb.installed"
    if sdl_has_switch2(dll) and (deps / "bin" / "libusb-1.0.dll").is_file() and installed(stamp, sdl_libusb_key()):
        log("SDL3.dll already has the Switch 2 driver (libusb)")
        return
    cmake = find_cmake()
    if not cmake:
        die("CMake not found: install it (winget install Kitware.CMake) or Visual Studio's C++ CMake tools")
    src = unpack("sdl3-src") / f"SDL3-{SDL_VERSION}"
    usb = unpack("libusb") / "ucrt64"
    build = CACHE / "build" / "sdl3-libusb"
    shutil.rmtree(build, ignore_errors=True)
    env = dict(os.environ, PATH=os.pathsep.join([str(llvm / "bin"), str(TC / "bin"), os.environ.get("PATH", "")]))
    cm = lambda p: Path(p).as_posix()   # CMake reads backslashes as escapes
    run([cmake, "-S", src, "-B", build, "-G", "Ninja", f"-DCMAKE_MAKE_PROGRAM={cm(TC / 'bin' / 'ninja.exe')}",
         f"-DCMAKE_C_COMPILER={cm(llvm / 'bin' / 'clang.exe')}", f"-DCMAKE_CXX_COMPILER={cm(llvm / 'bin' / 'clang++.exe')}",
         f"-DCMAKE_RC_COMPILER={cm(llvm / 'bin' / 'llvm-windres.exe')}", "-DCMAKE_BUILD_TYPE=Release",
         "-DSDL_SHARED=ON", "-DSDL_STATIC=OFF", "-DSDL_TEST_LIBRARY=OFF", "-DSDL_TESTS=OFF", "-DSDL_EXAMPLES=OFF",
         "-DSDL_HIDAPI=ON", "-DSDL_HIDAPI_LIBUSB=ON", "-DSDL_HIDAPI_LIBUSB_SHARED=ON", f"-DCMAKE_PREFIX_PATH={cm(usb)}",
         "-DSDL_VENDOR_INFO=Age of Decomp (HIDAPI libusb)"], env=env, capture=True)
    run([cmake, "--build", build, "--config", "Release"], env=env, capture=True)
    built = build / "SDL3.dll"
    if not sdl_has_switch2(built):
        die(f"{built} was built without the Switch 2 driver: libusb was not found by SDL's CMake")
    shutil.copy2(built, dll)
    shutil.copy2(build / "libSDL3.dll.a", deps / "lib" / "libSDL3.dll.a")
    shutil.copy2(usb / "bin" / "libusb-1.0.dll", deps / "bin" / "libusb-1.0.dll")
    p = usb / "share" / "licenses" / "libusb" / "COPYING"
    if p.is_file():
        shutil.copy2(p, lic / "libusb-COPYING.txt")   # LGPL-2.1: shipped as the unmodified DLL
    stamp.write_text(sdl_libusb_key(), encoding="utf-8")
    log(f"SDL {SDL_VERSION} with the Switch 2 driver (libusb) → {dll}")


def tar_members(name: str, dest: Path, members: list[str], strip: int = 0, from_list: Path | None = None) -> Path:
    """Extract only `members` (archive paths; GNU tar wildcards) of a pinned tarball into dest, or the exact entries of
    from_list (one per line, no recursion into directories). The system tar: tarfile reads the 2 GB LLVM .tar.xz far
    slower."""
    dest.mkdir(parents=True, exist_ok=True)
    cmd = ["tar", "-xf", str(fetch(name)), "-C", str(dest), f"--strip-components={strip}"]
    if from_list:
        cmd += ["--no-recursion", "-T", str(from_list)]
    else:
        cmd += ["--wildcards", "--no-wildcards-match-slash", *members]
    t0 = time.time()
    run(cmd)
    log(f"{name}: unpacked in {time.time() - t0:.0f} s")
    return dest


def materialize(root: Path, rel: str) -> Path | None:
    """The file behind root/rel, following its symlinks inside root (an absolute target is read from root), or None
    when it leads outside root or to nothing."""
    p = root / rel
    for _ in range(32):
        if not p.is_symlink():
            return p if p.is_file() else None
        target = os.readlink(p)
        p = Path(os.path.normpath(root / target.lstrip("/") if target.startswith("/") else p.parent / target))
        if not p.is_relative_to(root):
            return None
    return None


def dpkg_files(root: Path, package: str) -> list[str]:
    """The paths (without the leading /) the SDK's dpkg list names for package (amd64, else architecture-free)."""
    info = root / "var/lib/dpkg/info"
    for name in (f"{package}:amd64.list", f"{package}.list"):
        if (info / name).is_file():
            return [line.strip().lstrip("/") for line in (info / name).read_text(encoding="utf-8").splitlines()
                    if line.strip() and line.strip() != "/."]
    die(f"the sniper SDK has no package {package}")


def sysroot_keep(rel: str) -> str | None:
    """Where an SDK file goes in llvm/sysroot (GCC 14 moved to the usual places), or None when the build never reads
    it: documentation, gconv and the rest of glibc's runtime, static archives of the C library (the game links it
    dynamically), GCC's own headers and plugins."""
    if rel.startswith(("usr/share/", "etc/", "sbin/", "bin/", "usr/bin/", "usr/sbin/", "usr/libexec/")) or "/gconv/" in rel:
        return None
    for old, new in GCC14_MOVES:
        if rel.startswith(old):
            rest = rel[len(old):]
            if rest.startswith(("include/", "include-fixed/", "plugin/", "install-tools/", "libgcov", "libasan",
                                "libhwasan", "liblsan", "libtsan", "libubsan", "libgomp", "libitm", "libstdc++exp",
                                "libstdc++fs")):
                return None   # GCC's own headers, the sanitizers' and OpenMP's runtimes: the game uses none
            return new + rest
    if rel.startswith("usr/lib/gcc-14/"):
        return None   # gcc-14's own programs and documentation
    name = rel.rsplit("/", 1)[-1]
    if rel.startswith(("lib/x86_64-linux-gnu/", "lib64/")):   # libc6, libgcc-s1: only what a link reads
        return rel if name.startswith(GLIBC_LIBS + ("ld-linux", "libgcc_s")) and ".so" in name else None
    if rel.startswith("usr/lib/x86_64-linux-gnu/") and name.endswith(".a") and not name.endswith("_nonshared.a"):
        return None
    return rel


def deps_keep(rel: str) -> str | None:
    """Where an SDK file of DEPS_PACKAGES goes in deps/ (headers in include/, link libraries in lib/), or None."""
    for top in ("opus", "vpx"):
        if rel.startswith(f"usr/include/{top}/"):
            return "include/" + rel[len("usr/include/"):]
    name = rel.rsplit("/", 1)[-1]
    if rel.startswith("usr/lib/x86_64-linux-gnu/") and name in ("libopus.a", "libvpx.a", "libvulkan.so"):
        return "lib/" + name
    return None


def linux_sdk(dest: Path) -> None:
    """llvm/sysroot and deps/ from the sniper SDK: its dpkg lists first, then exactly the files of the packages, each
    one written as a plain file (a link's target under the link's name: the bundle and RecompLauncher's unpacker
    carry no links), the files that are only some link's target left out."""
    work = UNPACK / "sniper"
    stamp = work / ".unpacked"
    if not (stamp.is_file() and stamp.read_text() == PINS["sniper-sdk"][1]):
        shutil.rmtree(work, ignore_errors=True)
        tar_members("sniper-sdk", work, ["var/lib/dpkg/info/*.list"])
        docs = {f"usr/share/doc/{pkg}/copyright" for pkg in ("libc6", "linux-libc-dev", "gcc-14-base", "libopus0",
                                                             "libvpx6", "libvulkan1")}
        # The SDK image drops most documentation its dpkg lists still name: only what the archive holds is asked for
        held = set(run(["tar", "-tf", fetch("sniper-sdk")], capture=True).stdout.splitlines())
        wanted = sorted(rel for rel in {r for pkg in (*SYSROOT_PACKAGES, *DEPS_PACKAGES) for r in dpkg_files(work, pkg)
                                        if not r.startswith("usr/share/")} | docs if rel in held)
        listing = work / "members.txt"
        listing.write_text("\n".join(wanted) + "\n", encoding="utf-8")
        tar_members("sniper-sdk", work, [], from_list=listing)
        stamp.write_text(PINS["sniper-sdk"][1])
    sysroot, deps = dest / "llvm" / "sysroot", dest / "deps"
    for old in (sysroot, deps / "include" / "opus", deps / "include" / "vpx"):
        shutil.rmtree(old, ignore_errors=True)
    for name in ("libopus.a", "libvpx.a", "libvulkan.so"):
        (deps / "lib" / name).unlink(missing_ok=True)
    plan: dict[Path, Path] = {}
    targets: set[Path] = set()
    for packages, keep, top in ((SYSROOT_PACKAGES, sysroot_keep, sysroot), (DEPS_PACKAGES, deps_keep, deps)):
        for pkg in packages:
            for rel in dpkg_files(work, pkg):
                where = keep(rel)
                p = work / rel
                if where is None or not (p.is_file() or p.is_symlink()):
                    continue
                real = materialize(work, rel)
                if real is None:
                    continue   # a link to a file of a package we leave out
                if p.is_symlink():
                    targets.add(real)
                plan[top / where] = real
    n = 0
    for out, real in sorted(plan.items()):
        if real in targets and out.name == real.name:
            continue   # libc-2.31.so, libSDL3.so.0.4.18: read only through their links' names
        out.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(real, out)
        n += 1
    # The SDK's GCC 14 has no libgcc_s.so (its GCC 10 has the link): GCC's own link script, which clang's -lgcc_s reads
    (sysroot / "usr/lib/gcc/x86_64-linux-gnu/14/libgcc_s.so").write_text(
        "/* GNU ld script: the shared libgcc, and the static one for what only it has. */\n"
        "GROUP ( libgcc_s.so.1 -lgcc )\n", encoding="ascii")
    size = sum(p.stat().st_size for p in sysroot.rglob("*") if p.is_file())
    log(f"sniper SDK {SNIPER}: {n} files into llvm/sysroot ({size / 2**20:.0f} MiB) and deps/")


def linux_llvm(dest: Path) -> Path:
    """llvm/: clang, ld.lld and clang's own headers of LLVM's release (stripped), and the archiver and objcopy the
    toolchain step itself uses (tools/, never staged)."""
    top = f"LLVM-{LINUX_LLVM}-Linux-X64"
    major = LINUX_LLVM.split(".")[0]
    work = UNPACK / "llvm-linux"
    stamp = work / ".unpacked"
    if not (stamp.is_file() and stamp.read_text() == PINS["llvm-linux"][1]):
        shutil.rmtree(work, ignore_errors=True)
        tar_members("llvm-linux", work, [f"{top}/bin/clang-{major}", f"{top}/bin/lld", f"{top}/bin/llvm-ar",
                                         f"{top}/bin/llvm-objcopy", f"{top}/lib/clang/{major}/include"], strip=1)
        stamp.write_text(PINS["llvm-linux"][1])
    llvm = dest / "llvm"
    tools = dest / "tools"
    for d in (llvm / "bin", llvm / "lib", tools):
        shutil.rmtree(d, ignore_errors=True)
    tools.mkdir(parents=True)
    for name in ("llvm-ar", "llvm-objcopy"):
        shutil.copy2(work / "bin" / name, tools / name)
    (llvm / "bin").mkdir(parents=True)
    for src, name in ((f"clang-{major}", "clang"), ("lld", "ld.lld")):
        out = llvm / "bin" / name
        shutil.copy2(work / "bin" / src, out)
        run([tools / "llvm-objcopy", "--strip-unneeded", out])   # the release keeps its symbols: clang 270 MB
    shutil.copytree(work / "lib" / "clang" / major / "include", llvm / "lib" / "clang" / major / "include")
    version = run([llvm / "bin" / "clang", "--version"], capture=True).stdout.splitlines()[0]
    log(f"{version}, {sum(p.stat().st_size for p in llvm.rglob('*') if p.is_file()) / 2**20:.0f} MiB")
    return llvm


def linux_sdl(dest: Path) -> None:
    """deps/: SDL 3.4.20 (the Windows bundle's source) built in the sniper SDK's own image, with docker or podman (the
    image is the SDK tarball the sysroot comes from, imported once). glibc 2.31, and everything SDL can load at run
    time is loaded so (SDL_DEPS_SHARED, SDL's default): the library starts on a PC without Wayland, PipeWire, PulseAudio
    or libdecor, and HIDAPI uses libusb (wired Switch 2 pads) only where the PC has it."""
    deps = dest / "deps"
    work = UNPACK / "sdl3-linux"
    stamp = work / ".built"
    key = PINS["sdl3-src"][1] + PINS["sniper-sdk"][1]
    if not (stamp.is_file() and stamp.read_text() == key):
        tool = shutil.which("docker") or shutil.which("podman")
        if not tool:
            die("`toolchain` builds SDL in the Steam Runtime SDK's image: install docker or podman")
        image = f"aod-sniper-sdk:{SNIPER}"
        if subprocess.run([tool, "image", "inspect", image], capture_output=True).returncode:
            log(f"importing the sniper SDK as the image {image}")
            run([tool, "import", fetch("sniper-sdk"), image], capture=True)
        src = unpack("sdl3-src") / f"SDL3-{SDL_VERSION}"
        shutil.rmtree(work, ignore_errors=True)
        work.mkdir(parents=True)
        script = ("cmake -S /src -B /out/build -G Ninja -DCMAKE_BUILD_TYPE=Release -DSDL_SHARED=ON -DSDL_STATIC=OFF "
                  "-DSDL_TEST_LIBRARY=OFF -DSDL_TESTS=OFF -DSDL_EXAMPLES=OFF -DSDL_INSTALL=OFF -DSDL_DEPS_SHARED=ON "
                  "-DSDL_HIDAPI=ON -DSDL_HIDAPI_LIBUSB=ON -DSDL_HIDAPI_LIBUSB_SHARED=ON "
                  "'-DSDL_VENDOR_INFO=Age of Decomp (Steam Runtime 3)' > /out/cmake.log && cmake --build /out/build "
                  "> /out/build.log")
        run([tool, "run", "--rm", "--network=none", "-u", f"{os.getuid()}:{os.getgid()}",
             "-v", f"{src.resolve()}:/src:ro", "-v", f"{work.resolve()}:/out", image, "/bin/sh", "-c", script])
        stamp.write_text(key)
    built = materialize(work, "build/libSDL3.so.0")
    if built is None or not sdl_has_switch2(built):
        die(f"{work / 'build'}: no libSDL3.so.0 with the Switch 2 driver (libusb); see {work / 'cmake.log'}")
    needed, newest = elf_needs(built)
    hard = sorted(set(needed) - {"libc.so.6", "libm.so.6", "libdl.so.2", "libpthread.so.0", "librt.so.1",
                                 "ld-linux-x86-64.so.2"})
    if hard or newest > (2, 31):
        die(f"{built.name} needs {', '.join(hard) or 'only the C library'} and glibc {'.'.join(map(str, newest))}")
    (deps / "lib").mkdir(parents=True, exist_ok=True)
    for name in ("libSDL3.so", "libSDL3.so.0"):   # the link's name, and the one the game loads beside it
        shutil.copy2(built, deps / "lib" / name)
    shutil.rmtree(deps / "include" / "SDL3", ignore_errors=True)
    shutil.copytree(unpack("sdl3-src") / f"SDL3-{SDL_VERSION}" / "include" / "SDL3", deps / "include" / "SDL3")
    log(f"SDL {SDL_VERSION}: {built.stat().st_size / 2**20:.1f} MiB, needs {', '.join(needed)}")


def linux_deps(dest: Path) -> None:
    """deps/: the Vulkan headers and VMA (the Windows bundle's versions), zstd built static with the toolchain itself;
    bin/ninja."""
    deps = dest / "deps"
    for sub in ("include/vulkan", "include/vk_video", "include/vma"):
        shutil.rmtree(deps / sub, ignore_errors=True)
    vk = UNPACK / "vulkan-headers"
    shutil.rmtree(vk, ignore_errors=True)
    tar_members("vulkan-headers-src", vk, ["Vulkan-Headers-1.4.363/include", "Vulkan-Headers-1.4.363/LICENSE.md"],
                strip=1)
    for sub in ("vulkan", "vk_video"):
        shutil.copytree(vk / "include" / sub, deps / "include" / sub)
    (deps / "include" / "vma").mkdir(parents=True)
    shutil.copy2(fetch("vma-header"), deps / "include" / "vma" / "vk_mem_alloc.h")
    zs = UNPACK / "zstd"
    shutil.rmtree(zs, ignore_errors=True)
    tar_members("zstd-src", zs, ["zstd-1.5.7"], strip=1)
    import shlex   # make hands CC and AR to a shell: a cache folder with a space in its name stays one word
    cc = (f"{shlex.quote(str(dest / 'llvm/bin/clang'))} --target=x86_64-linux-gnu "
          f"--sysroot={shlex.quote(str(dest / 'llvm/sysroot'))} -fuse-ld=lld")
    lib = zs / "lib"
    run(["make", "-C", lib, f"-j{os.cpu_count() or 4}", "libzstd.a", f"CC={cc}",
         f"AR={shlex.quote(str(dest / 'tools/llvm-ar'))}", "ZSTD_LEGACY_SUPPORT=0", "MOREFLAGS=-O2"], capture=True)
    (deps / "lib").mkdir(parents=True, exist_ok=True)
    shutil.copy2(lib / "libzstd.a", deps / "lib" / "libzstd.a")
    for name in ("zstd.h", "zstd_errors.h"):
        shutil.copy2(lib / name, deps / "include" / name)
    (dest / "bin").mkdir(exist_ok=True)
    with zipfile.ZipFile(fetch("ninja-linux")) as z:
        (dest / "bin" / "ninja").write_bytes(z.read("ninja"))
    os.chmod(dest / "bin" / "ninja", 0o755)


def linux_licenses(dest: Path) -> None:
    lic = dest / "licenses"
    shutil.rmtree(lic, ignore_errors=True)
    lic.mkdir()
    doc = UNPACK / "sniper" / "usr/share/doc"
    for src, name in ((fetch("llvm-license"), "llvm-LICENSE.txt"), (doc / "libc6/copyright", "glibc-copyright.txt"),
                      (doc / "linux-libc-dev/copyright", "linux-libc-dev-copyright.txt"),
                      (doc / "gcc-14-base/copyright", "gcc-14-copyright.txt"),
                      (unpack("sdl3-src") / f"SDL3-{SDL_VERSION}" / "LICENSE.txt", "SDL3-LICENSE.txt"),
                      (doc / "libopus0/copyright", "opus-copyright.txt"),
                      (doc / "libvpx6/copyright", "libvpx-copyright.txt"),
                      (doc / "libvulkan1/copyright", "vulkan-loader-copyright.txt"),
                      (UNPACK / "vulkan-headers/LICENSE.md", "vulkan-headers-LICENSE.txt"),
                      (UNPACK / "zstd/LICENSE", "zstd-LICENSE.txt"),
                      (ROOT / "packaging/licenses/libvpx-PATENTS.txt", "libvpx-PATENTS.txt"),
                      (ROOT / "packaging/licenses/ninja-LICENSE.txt", "ninja-LICENSE.txt")):
        if src.is_file():
            shutil.copy2(src, lic / name)
    head = (dest / "deps/include/vma/vk_mem_alloc.h").read_text(encoding="utf-8").split("*/")[0]
    (lic / "vma-LICENSE.txt").write_text(head.lstrip("/*").strip() + "\n", encoding="utf-8")
    for py_lic in sorted((dest / "python" / "lib").glob("python3.*/LICENSE.txt"))[:1]:
        shutil.copy2(py_lic, lic / "python-LICENSE.txt")
    missing = [name for name in LINUX_LICENSES if not (lic / name).is_file()]
    if missing:
        die(f"the Linux toolchain has no licence text for {', '.join(missing)}")


PROBE_C = r"""#include <SDL3/SDL.h>
#include <vulkan/vulkan.h>
#include <opus/opus.h>
#include <vpx/vpx_decoder.h>
#include <vpx/vp8dx.h>
#include <zstd.h>
#include <vk_mem_alloc.h>
#include <pthread.h>
#include <math.h>
#include <linux/userfaultfd.h>
#include <stdio.h>
int cxx(void);
int main(void) {
    printf("SDL %d, zstd %u, %s, %s, C++ %d, %.3f\n", SDL_GetVersion(), ZSTD_versionNumber(), opus_get_version_string(),
           vpx_codec_iface_name(vpx_codec_vp9_dx()), cxx(), sqrt(2.0));
    return (void *)&vkGetInstanceProcAddr == NULL;
}
"""
PROBE_CXX = r"""#include <bit>
#include <span>
#include <atomic>
#include <unordered_map>
#include <string>
#include <mutex>
#include <optional>
static std::mutex m;
extern "C" int cxx(void) {
    std::lock_guard<std::mutex> g(m);
    std::unordered_map<std::string, int> u{{"a", 1}};
    int a[2] = {1, 2};
    std::span<int> s(a);
    std::optional<float> f = 1.0f;
    std::atomic<int> n{std::bit_cast<int>(*f) != 0};
    try { throw 1; } catch (int) { n++; }
    return n + u["a"] + (int)s.size();
}
"""


def elf_needs(path: Path) -> tuple[list[str], tuple[int, ...]]:
    """An ELF file's DT_NEEDED libraries and the newest GLIBC_ symbol version it asks for (binutils' readelf)."""
    import re
    dyn = run(["readelf", "-d", "-W", path], capture=True).stdout
    ver = run(["readelf", "-V", "-W", path], capture=True).stdout
    needed = re.findall(r"\(NEEDED\)\s+Shared library: \[([^\]]+)\]", dyn)
    newest = max((tuple(map(int, v.split("."))) for v in re.findall(r"GLIBC_(\d+(?:\.\d+)+)", ver)), default=(0,))
    return needed, newest


def linux_probe(dest: Path) -> None:
    """One C and one C++20 program compiled and linked with the toolchain alone, the way build_game.py links the game:
    it may name no glibc above the sysroot's 2.31 and no library besides SDL3 (beside it), Vulkan and the C runtime,
    and it must run here."""
    import tempfile
    llvm, deps = dest / "llvm", dest / "deps"
    with tempfile.TemporaryDirectory(prefix="aod-probe-") as tmp:
        t = Path(tmp)
        (t / "c.c").write_text(PROBE_C, encoding="ascii")
        (t / "x.cpp").write_text(PROBE_CXX, encoding="ascii")
        cc = [llvm / "bin/clang", "--target=x86_64-linux-gnu", f"--sysroot={llvm / 'sysroot'}", "-O1",
              "-march=x86-64-v3", "-I", deps / "include", "-I", deps / "include/vma"]
        run([*cc, "-c", t / "c.c", "-o", t / "c.o"])
        run([*cc, "-std=c++20", "-c", t / "x.cpp", "-o", t / "x.o"])
        run([*cc, "-fuse-ld=lld", "-no-pie", "-Wl,-rpath,$ORIGIN", t / "c.o", t / "x.o", "-L", deps / "lib",
             "-lvulkan", "-lSDL3", "-l:libopus.a", "-l:libvpx.a", "-l:libzstd.a", "-l:libstdc++.a", "-lm",
             "-lpthread", "-ldl", "-o", t / "probe"])
        shutil.copy2(deps / "lib/libSDL3.so.0", t / "libSDL3.so.0")
        needed, newest = elf_needs(t / "probe")
        unknown = sorted(set(needed) - {"libSDL3.so.0", "libvulkan.so.1", "libm.so.6", "libc.so.6", "libgcc_s.so.1",
                                        "libpthread.so.0", "libdl.so.2", "ld-linux-x86-64.so.2"})
        if unknown or newest > (2, 31):
            die(f"toolchain probe: needs {', '.join(unknown) or 'only known libraries'}, "
                f"glibc {'.'.join(map(str, newest))}")
        out = run([t / "probe"], capture=True, check=False)
        if out.returncode not in (0, 1):   # 1: no Vulkan loader on this PC, the program still ran
            die(f"toolchain probe exit {out.returncode}: {out.stderr.strip()[-400:]}")
        log(f"toolchain probe: {out.stdout.strip()} (needs {', '.join(needed)}; glibc {'.'.join(map(str, newest))})")


def toolchain_linux() -> None:
    for tool in ("tar", "make", "readelf"):
        if not shutil.which(tool):
            die(f"`toolchain` needs {tool} on PATH (GNU tar, make, binutils' readelf; and docker or podman)")
    missing = [f"{tool} ({why})" for tool, why in LINUX_DEV_TOOLS.items() if not shutil.which(tool)]
    TC.mkdir(parents=True, exist_ok=True)
    # Python: relocatable CPython + the generator's packages (the staged bundle and the AppImage carry it)
    py = TC / "python"
    exe = py / "bin" / "python3"
    py_stamp = TC / "python.installed"
    if not exe.is_file() or not installed(py_stamp, PINS["python-linux"][1]):
        shutil.rmtree(py, ignore_errors=True)
        n = copy_tree(unpack("python-linux") / "python", py)
        py_stamp.write_text(PINS["python-linux"][1], encoding="utf-8")
        log(f"python-build-standalone: {n} files")

    def install_linux_packages() -> None:
        if subprocess.run([exe, "-I", "-m", "pip", "--version"], capture_output=True).returncode:
            run([exe, "-I", "-m", "ensurepip"])
        run([exe, "-I", "-m", "pip", "install", "--disable-pip-version-check", "--no-warn-script-location",
             "--only-binary", ":all:", "--require-hashes", "-r", PY_REQUIREMENTS])
    python_packages(exe, TC / "python-packages.installed", install_linux_packages)
    check = run([exe, "-I", "-c", "import sys, numpy, lz4.block, zstandard, capstone; "
                 "print('python', sys.version.split()[0], 'numpy', numpy.__version__)"], capture=True)
    log(check.stdout.strip())
    # The compiler and the libraries the game links: what the bundle carries to the player's PC
    linux_llvm(TC)
    linux_sdk(TC)
    linux_sdl(TC)
    linux_deps(TC)
    linux_licenses(TC)
    linux_probe(TC)
    if missing:
        log("not on PATH, needed by the developer steps only: " + ", ".join(missing))
    log(f"toolchain ready in {TC} (LLVM {LINUX_LLVM}, sniper {SNIPER} sysroot, SDL3, Vulkan, Ninja)")


def cmd_toolchain(_args) -> None:
    toolchain_windows() if WIN else toolchain_linux()


# ── shader translator ────────────────────────────────────────────────────────────────────────────────────────────────

def cmd_translator(args) -> None:
    dotnet = TC / "DotNet" / "dotnet.exe" if WIN else Path(shutil.which("dotnet") or "dotnet")
    python = TC / "python" / ("python.exe" if WIN else "bin/python3")
    env = dict(os.environ, PYTHONUTF8="1", HWAOI_NUGET_CACHE=str(TC / "NuGet"))
    env.pop("HWAOI_TOOLS", None)
    if XLATE_OUT.exists():
        shutil.rmtree(XLATE_OUT)  # the builder refuses to overwrite a deployment
    run([python, "-E", "-s", ENGINE / "tools" / "build_xlate_hwaoi.py", "--dotnet", dotnet, "--out", XLATE_OUT,
         "--online", "--no-vendor-reference"], cwd=ENGINE, env=env)
    log(f"translator in {XLATE_OUT}")


# ── staging the portable bundle ──────────────────────────────────────────────────────────────────────────────────────

# The engine folders whose files make the game: the staged bundle's revision is taken over these alone.
ENGINE_PARTS = ("compiler", "config", "recomp", "runtime", "tools")


def source_files():
    """The engine files a bundle carries: what git tracks under engine/source (so nothing ignored or left over from
    a build gets in), paths relative to it. Not its tests and docs: the setup copies the bundle's sources into every
    game folder, and CI runs the tests from the repository."""
    raw = git("ls-files", "-z", "--", ".").stdout
    for name in sorted(set(raw.split("\0")) - {""}):
        top = name.split("/", 1)[0]
        if top in ENGINE_PARTS or name in (
                "LICENSE", "LICENSE-MIT", "THIRD_PARTY_NOTICES.md", "README.md", "VERSION"):
            yield name


def engine_revision(files: dict[str, str]) -> str:
    """One id for the engine a bundle carries: the staged files of compiler/, config/, recomp/, runtime/ and tools/
    with their hashes. Tests, notices and VERSION stay out, so a release that changes only those does not tell
    players that the engine changed (the launcher offers a rebuild when this id is not the build's)."""
    h = hashlib.sha256()
    for path in sorted(files):
        parts = path.split("/")
        if parts[0] == "source" and len(parts) > 2 and parts[1] in ENGINE_PARTS:
            h.update(f"{path}\t{files[path]}\n".encode("utf-8"))
    return h.hexdigest()[:40]


def bundle_python(root: Path) -> Path:
    return root / "python" / ("python.exe" if WIN else "bin/python3")


def linux_python_skip(rel: str) -> bool:
    """Parts of python-build-standalone the engine never uses: Tcl/Tk, IDLE, pip, the test suite, headers, and share/
    (terminfo names that differ only in case turn into symlink loops on a case-insensitive drive, e.g. under WSL)."""
    parts = rel.split("/")
    if parts[0] in ("share", "include") or rel.startswith(("bin/idle", "bin/pip", "lib/tcl", "lib/tk", "lib/itcl",
                                                            "lib/thread", "lib/libtcl", "lib/libtk", "lib/pkgconfig")):
        return True
    if len(parts) > 2 and parts[0] == "lib" and parts[1].startswith("python3."):
        if parts[2] in ("test", "idlelib", "tkinter", "turtledemo", "ensurepip") or rel.split("/")[-1].startswith("_tkinter"):
            return True
        return parts[2] == "site-packages" and len(parts) > 3 and (parts[3] == "pip" or parts[3].startswith("pip-"))
    return False


def cmd_stage(args) -> None:
    """The full portable compiler: Python, the translator, LLVM (Windows: llvm-mingw; Linux: clang, lld and the sniper
    sysroot), deps and Ninja."""
    dirty = bool(git("status", "--porcelain", "--", ".").stdout.strip())
    if dirty and not args.allow_dirty:
        die("engine/source has uncommitted changes; commit them or pass --allow-dirty")
    if dirty:
        log("staging engine/source with uncommitted changes (--allow-dirty)")
    needs = [bundle_python(TC), XLATE_OUT / XLATE_EXE, TC / "llvm" / "bin" / ("clang.exe" if WIN else "clang")]
    for need in needs:
        if not need.is_file():
            die(f"missing {need}: run toolchain / translator first")
    if STAGE.is_symlink():   # the stage kept on another drive (WSL: ext4 instead of the shared NTFS folder): emptied
        if any(STAGE.iterdir()) and not (STAGE / "bundle.json").is_file() and not (STAGE / "source").is_dir():
            die(f"{STAGE} links to {STAGE.resolve()}, which holds something that is not a staged bundle")
        for child in STAGE.iterdir():
            shutil.rmtree(child) if child.is_dir() and not child.is_symlink() else child.unlink()
    elif STAGE.exists():
        shutil.rmtree(STAGE)
    STAGE.mkdir(parents=True, exist_ok=True)
    n = 0
    for rel in source_files():
        dst = STAGE / "source" / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ENGINE / rel, dst)
        n += 1
    # The runtime generator reads source/VERSION. Keep it tied to the compiler release,
    # rather than shipping the engine fork's development version in the menu.
    version = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
    (STAGE / "source/runtime").mkdir(parents=True, exist_ok=True)
    (STAGE / "source/VERSION").write_text(version + "\n", encoding="utf-8")
    log(f"source: {n} files")
    copy_tree(TC / "python", STAGE / "python", lambda rel: "__pycache__" in rel or rel.endswith(".pyc") or
              (not WIN and linux_python_skip(rel)))
    # Not the .NET runtime's LTTng tracing provider: coreclr loads it only where liblttng-ust is installed, and the
    # translator is never traced
    copy_tree(XLATE_OUT, STAGE / "translator", lambda rel: rel.endswith(("build-report.json", ".pdb")) or
              rel == "libcoreclrtraceptprovider.so")
    if WIN:
        copy_tree(TC / "llvm", STAGE / "llvm")
        copy_tree(TC / "deps", STAGE / "deps")
        (STAGE / "bin").mkdir()
        shutil.copy2(TC / "bin" / "ninja.exe", STAGE / "bin" / "ninja.exe")
        shutil.copy2(TC / "bin" / "zlib1.dll", STAGE / "bin" / "zlib1.dll")
        copy_tree(TC / "licenses", STAGE / "licenses")
        nintendo = STAGE / "nintendo"
        dotnet = TC / "DotNet/dotnet.exe"
        run([dotnet, "publish", ROOT / "apps/nintendo-bridge/AgeOfDecomp.Nintendo.csproj", "-c", "Release",
             "-r", "win-x64", "--self-contained", "true", "-o", nintendo, "-p:RestoreLockedMode=true",
             f"-p:Version={version}"], cwd=ROOT)
        deps = json.loads((nintendo / "AgeOfDecomp.Nintendo.deps.json").read_text(encoding="utf-8"))
        runtime = next(key for key in deps["libraries"] if key.startswith("runtimepack.Microsoft.NETCore.App.Runtime.win-x64/"))
        pack = "microsoft.netcore.app.runtime.win-x64/" + runtime.split("/", 1)[1]
        assets = json.loads((ROOT / "apps/nintendo-bridge/obj/project.assets.json").read_text(encoding="utf-8"))
        for name in ("LICENSE.TXT", "THIRD-PARTY-NOTICES.TXT"):
            notice = next((Path(folder) / pack / name for folder in assets["packageFolders"]
                           if (Path(folder) / pack / name).is_file()), None)
            if not notice:
                die(f".NET runtime pack is missing {name}")
            shutil.copy2(notice, STAGE / "licenses" / f"dotnet-{name}")
        run([TC / "llvm/bin/clang.exe", "-shared", "-O2", "-Wall", "-Wextra", "-Werror", "-DAOD_NINTENDO_DLL",
             "-D_WIN32_WINNT=0x0A00", "-I", TC / "deps/include", ENGINE / "runtime" / "aod_nintendo.c",
             "-L", TC / "deps/lib", "-lSDL3", "-o", nintendo / "aod_nintendo.dll"], cwd=ROOT)
        run([nintendo / "AgeOfDecomp.Nintendo.exe", "--self-test"], cwd=ROOT)
        # DLAA (runtime/aod_dlss.cpp): the launcher points TOTK_DLSS_DIR at <bundle>/streamline. The player's own
        # nvngx_dlss.dll comes from RecompLauncher's nvidia.dlss input or the player's PC, never from us.
        streamline = STAGE / "streamline"
        streamline.mkdir()
        with zipfile.ZipFile(fetch("streamline-sdk")) as z:
            for name in STREAMLINE_DLLS:
                (streamline / name).write_bytes(z.read(f"bin/x64/{name}"))
            (STAGE / "licenses" / "streamline-LICENSE.txt").write_bytes(z.read("license.txt"))
        missing = [name for name in (*WINDOWS_LICENSES, "dotnet-LICENSE.TXT", "dotnet-THIRD-PARTY-NOTICES.TXT",
                                     "streamline-LICENSE.txt") if not (STAGE / "licenses" / name).is_file()]
        if missing:
            die(f"the bundle would ship without {', '.join(missing)}: run `toolchain` again")
    else:
        copy_tree(TC / "llvm", STAGE / "llvm")
        copy_tree(TC / "deps", STAGE / "deps")
        (STAGE / "bin").mkdir()
        shutil.copy2(TC / "bin" / "ninja", STAGE / "bin" / "ninja")
        copy_tree(TC / "licenses", STAGE / "licenses")
        missing = [name for name in LINUX_LICENSES if not (STAGE / "licenses" / name).is_file()]
        if missing:
            die(f"the bundle would ship without {', '.join(missing)}: run `toolchain` again")
    files = {p.relative_to(STAGE).as_posix(): sha256(p) for p in sorted(STAGE.rglob("*")) if p.is_file()}
    manifest = {"format": "hwaoi-compiler-v1", "version": version, "source_revision": engine_revision(files),
                "aod_prepare_version": int((ROOT / "packaging/prepare-version.txt").read_text(encoding="ascii")),
                "files": files, "game_content_included": False}
    if dirty:
        manifest["dirty"] = True
    (STAGE / "bundle.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    size = sum((STAGE / p).stat().st_size for p in files)
    log(f"staged {len(files)} files, {size / 2**20:.0f} MiB in {STAGE}")
    # -B: the check writes no __pycache__ into the bundle it has just hashed (release.py also drops any). A bundle the
    # engine refuses keeps no bundle.json: `status` and release.py then see no bundle at all.
    checked = run([bundle_python(STAGE), "-B", "-E", "-s", STAGE / "source" / "compiler" / "backend.py",
                   "check", "--bundle", STAGE], cwd=STAGE, check=False)
    if checked.returncode:
        (STAGE / "bundle.json").unlink()
        die("the engine's own check refused the staged bundle (above); bundle.json removed")


# ── developer game build ─────────────────────────────────────────────────────────────────────────────────────────────

def cmd_game(args) -> None:
    nup = Path(args.nup).resolve(strict=True)
    out = Path(args.out).resolve()
    python = bundle_python(STAGE)
    if not python.is_file():
        die("run `engine.py stage` first")
    cmd = [python, "-E", "-s", STAGE / "source" / "compiler" / "backend.py", "build", "--bundle", STAGE,
           "--nup", nup, "--output", out, "--jobs", str(args.jobs), "--resolution", args.resolution]
    started = time.time()
    run(cmd, cwd=STAGE)
    log(f"game built in {(time.time() - started) / 60:.1f} min -> {out}")


def cmd_status(_args) -> None:
    bundle = json.loads((STAGE / "bundle.json").read_text(encoding="utf-8")) if (STAGE / "bundle.json").is_file() else {}
    rows = [("toolchain", "ok" if (TC / "llvm/bin/clang.exe" if WIN else bundle_python(TC)).exists() else "missing"),
            ("engine source", f"{sum(1 for _ in source_files())} files" if ENGINE.is_dir() else "missing"),
            ("staged engine", bundle.get("source_revision", "missing")[:12] + (" (dirty)" if bundle.get("dirty") else "")),
            ("translator", "ok" if (XLATE_OUT / XLATE_EXE).is_file() else "missing"),
            ("bundle", "ok" if (STAGE / "bundle.json").is_file() else "missing")]
    for k, v in rows:
        print(f"{k:16s} {v}")


def main() -> None:
    for stream in (sys.stdout, sys.stderr):   # paths and notes can hold characters a Windows console code page lacks
        stream.reconfigure(errors="replace")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("toolchain")
    sub.add_parser("translator")
    s = sub.add_parser("stage")
    s.add_argument("--allow-dirty", action="store_true")
    g = sub.add_parser("game")
    g.add_argument("--nup", required=True)
    g.add_argument("--out", default=str(ROOT / "work" / ("game" if WIN else "game-linux")))
    g.add_argument("--jobs", type=int, default=0)
    g.add_argument("--resolution", default="auto", choices=("auto", "1280x720", "1920x1080", "2560x1440", "3840x2160"))
    sub.add_parser("status")
    args = ap.parse_args()
    {"toolchain": cmd_toolchain, "translator": cmd_translator, "stage": cmd_stage, "game": cmd_game,
     "status": cmd_status}[args.cmd](args)


if __name__ == "__main__":
    main()
