#!/usr/bin/env python3
"""Recreates refs/: read-only upstream checkouts used as reference (never built from, never edited).

  py tools/fetch_refs.py            clone what is missing (shallow)
  py tools/fetch_refs.py --update   also fast-forward existing clones
"""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REFS = {
    "hwaoi-recomp": ("https://github.com/Alano6678/hwaoi-recomp.git", ["--filter=blob:none"]),
    "dkb-pc-recomp": ("https://github.com/Lucas-blip-png/dkb-pc-recomp.git", ["--depth", "1"]),
    "mkw-recomp": ("https://github.com/adriadam10/mkw-recomp.git", ["--depth", "1"]),
    "Switch2RECOMP": ("https://github.com/SwitchRECOMP-Dev/Switch2RECOMP.git", ["--depth", "1"]),  # AGPL: read only
    "nup": ("https://github.com/a55uka/nup.git", ["--depth", "1"]),
    "RecompLauncher": ("https://github.com/elripalda/RecompLauncher.git", ["--depth", "1"]),
    # Sources compared in docs/CODE_PROVENANCE.md (full history: the report cites commits and `git log -S`).
    "SavageWindRecomp": ("https://github.com/Revivedx/SavageWindRecomp.git", ["--filter=blob:none"]),
    "LASRecomp": ("https://github.com/peiikai/LASRecomp.git", ["--filter=blob:none"]),
    "totk-pc-recomp": ("https://github.com/valentderah/totk-pc-recomp.git", ["--filter=blob:none"]),
    "Deluxe8": ("https://github.com/iCosiSenpai/Deluxe8.git", ["--filter=blob:none"]),
    # G-2/G-3 (research/dlss.md §8): FSR through the FidelityFX API, pinned tag, only its C headers, licence and the
    # two Vulkan DLLs (MIT per Kits/FidelityFX/docs/license.md). Never built from in place, never committed.
    "FidelityFX-SDK": ("https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK.git",
                       ["--depth", "1", "--branch", "v2.4.0", "--filter=blob:none", "--no-checkout"]),
}

# Partial checkouts: name -> the paths to check out (git sparse-checkout, non-cone patterns).
SPARSE = {
    "FidelityFX-SDK": ["/Kits/FidelityFX/api/", "/Kits/FidelityFX/upscalers/include/",
                       "/Kits/FidelityFX/framegeneration/include/", "/Kits/FidelityFX/docs/license.md",
                       "/Kits/FidelityFX/readme.md", "/Kits/FidelityFX/signedbin/amd_fidelityfx_loader_vk.dll",
                       "/Kits/FidelityFX/signedbin/amd_fidelityfx_upscaler_vk.dll", "/3rdpartynotice.md",
                       "/readme.md"],
}


def main() -> int:
    (ROOT / "refs").mkdir(exist_ok=True)
    for name, (url, opts) in REFS.items():
        dest = ROOT / "refs" / name
        if not dest.is_dir():
            subprocess.run(["git", "clone", "-q", *opts, url, str(dest)], check=True)
            if name in SPARSE:
                subprocess.run(["git", "-C", str(dest), "sparse-checkout", "set", "--no-cone", *SPARSE[name]], check=True)
                subprocess.run(["git", "-C", str(dest), "checkout", "-q"], check=True)
        elif name in SPARSE:
            pass   # a pinned tag: refetch by deleting the folder
        elif "--update" in sys.argv:
            subprocess.run(["git", "-C", str(dest), "pull", "-q", "--ff-only"], check=False)
        head = subprocess.run(["git", "-C", str(dest), "log", "-1", "--format=%h %cs"], capture_output=True, text=True).stdout.strip()
        print(f"{name:16s} {head}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
