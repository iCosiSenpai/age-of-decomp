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
}


def main() -> int:
    (ROOT / "refs").mkdir(exist_ok=True)
    for name, (url, opts) in REFS.items():
        dest = ROOT / "refs" / name
        if not dest.is_dir():
            subprocess.run(["git", "clone", "-q", *opts, url, str(dest)], check=True)
        elif "--update" in sys.argv:
            subprocess.run(["git", "-C", str(dest), "pull", "-q", "--ff-only"], check=False)
        head = subprocess.run(["git", "-C", str(dest), "log", "-1", "--format=%h %cs"], capture_output=True, text=True).stdout.strip()
        print(f"{name:16s} {head}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
