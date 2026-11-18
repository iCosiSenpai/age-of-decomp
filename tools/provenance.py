#!/usr/bin/env python3
"""Source comparison for docs/CODE_PROVENANCE.md: how much of a local file matches a file of another project.

  py -I tools/provenance.py compare <ours> <theirs> [--min 4]     similarity + shared runs of lines
  py -I tools/provenance.py tree <our dir> <their dir> [--ext .c,.h] same-named files in two trees, one line each

Lines are compared after normalisation: surrounding whitespace dropped, runs of spaces collapsed, blank lines and
lines that are only a brace removed (both files keep their real line numbers). "same" counts normalised lines of ours
that occur in the other file's matched runs; "runs" are maximal blocks of at least --min consecutive equal lines,
printed with the line ranges on both sides. Stdlib only, so anyone can repeat a result from the report.
"""
from __future__ import annotations

import argparse
import difflib
import hashlib
import re
import sys
from pathlib import Path

TRIVIAL = {"", "{", "}", "};", "})", "});", "*/", "/*", "//"}


def norm_lines(path: Path) -> list[tuple[int, str]]:
    text = path.read_bytes().decode("utf-8", errors="replace").replace("\r\n", "\n")
    out = []
    for n, line in enumerate(text.split("\n"), 1):
        s = re.sub(r"\s+", " ", line.strip())
        if s not in TRIVIAL:
            out.append((n, s))
    return out


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes().replace(b"\r\n", b"\n")).hexdigest()


def compare(ours: Path, theirs: Path, min_run: int) -> dict:
    a, b = norm_lines(ours), norm_lines(theirs)
    sm = difflib.SequenceMatcher(None, [s for _, s in a], [s for _, s in b], autojunk=False)
    runs = [m for m in sm.get_matching_blocks() if m.size >= min_run]
    same = sum(m.size for m in sm.get_matching_blocks())
    return {
        "ours_lines": len(a), "theirs_lines": len(b), "same": same,
        "pct_ours": 100.0 * same / len(a) if a else 0.0, "ratio": sm.ratio(),
        "identical": sha(ours) == sha(theirs),
        "runs": [(a[m.a][0], a[m.a + m.size - 1][0], b[m.b][0], b[m.b + m.size - 1][0], m.size) for m in runs],
    }


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("compare")
    c.add_argument("ours", type=Path)
    c.add_argument("theirs", type=Path)
    c.add_argument("--min", type=int, default=4)
    t = sub.add_parser("tree")
    t.add_argument("ours", type=Path)
    t.add_argument("theirs", type=Path)
    t.add_argument("--ext", default=".c,.h,.cpp,.mm,.py,.rs,.ts,.svelte,.css,.js,.mjs")
    t.add_argument("--min", type=int, default=4)
    args = p.parse_args()
    if args.cmd == "compare":
        r = compare(args.ours, args.theirs, args.min)
        print(f"{args.ours} vs {args.theirs}")
        print(f"  normalised lines: ours {r['ours_lines']}, theirs {r['theirs_lines']}; byte-identical: {r['identical']}")
        print(f"  matched lines {r['same']} = {r['pct_ours']:.1f}% of ours; difflib ratio {r['ratio']:.3f}")
        for a0, a1, b0, b1, n in r["runs"]:
            print(f"  run of {n:4d}: ours {a0}-{a1}  theirs {b0}-{b1}")
        return 0
    exts = set(args.ext.split(","))
    theirs = {}
    for f in args.theirs.rglob("*"):
        if f.is_file() and f.suffix in exts:
            theirs.setdefault(f.name, []).append(f)
    for f in sorted(args.ours.rglob("*")):
        if not f.is_file() or f.suffix not in exts or f.name not in theirs:
            continue
        for g in theirs[f.name]:
            r = compare(f, g, args.min)
            longest = max((x[4] for x in r["runs"]), default=0)
            print(f"{r['pct_ours']:5.1f}% ident={int(r['identical'])} longest={longest:4d} "
                  f"{f.relative_to(args.ours)}  <->  {g.relative_to(args.theirs)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
