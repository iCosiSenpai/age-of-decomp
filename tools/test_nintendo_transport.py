"""Verify the integrated Windows BLE transport without hardware or game data."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TC = ROOT / ".cache/toolchain/windows"
OUT = ROOT / ".cache/nintendo-transport-test"


def run(args):
    subprocess.run([str(a) for a in args], cwd=ROOT, check=True)


def main():
    if os.name != "nt":
        sys.exit("The BLE transport is Windows-only; run this check on Windows.")
    OUT.mkdir(parents=True, exist_ok=True)
    run([TC / "DotNet/dotnet.exe", "publish", ROOT / "apps/nintendo-bridge/AgeOfDecomp.Nintendo.csproj",
         "-c", "Release", "-r", "win-x64", "--self-contained", "true", "-o", OUT, "-p:RestoreLockedMode=true"])
    helper = OUT / "AgeOfDecomp.Nintendo.exe"
    run([helper, "--self-test"])
    run([TC / "llvm/bin/clang.exe", "-O2", "-g", "-Wall", "-Wextra", "-Werror", "-D_WIN32_WINNT=0x0A00",
         "-I", TC / "deps/include", ROOT / "engine/tests/nintendo_transport.c", "-L", TC / "deps/lib", "-lSDL3",
         "-o", OUT / "nintendo-test.exe"])
    shutil.copy2(TC / "deps/bin/SDL3.dll", OUT / "SDL3.dll")
    run([OUT / "nintendo-test.exe", helper])


if __name__ == "__main__":
    main()
