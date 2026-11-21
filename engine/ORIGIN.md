# Where the engine comes from

`engine/source/` is Age of Decomp's engine: the compiler that turns the player's own copy of the game into a native
program (`compiler/`, `recomp/`, `tools/`) and the runtime that program links against (`runtime/`). It is developed
here, in this repository.

## Origin

It was imported on 2026-11-21 from **HWAoI Recomp**, https://github.com/Alano6678/hwaoi-recomp, at commit
`c6c96e61b935fa610adf626c299553a568908fef` (2026-11-20), together with the 36 changes Age of Decomp had made on top of
it. Until then this repository kept only a pin of that project and a queue of patches (`engine/upstream.json` and
`engine/patches/` in the history up to 0.1.2).

HWAoI Recomp is itself an adaptation of **DKB Recomp** (https://github.com/Lucas-blip-png/dkb-pc-recomp, branch
`bananza`, commit `42fcd9d78b4b44c2831b68ba21e5807d74a96f45`, recorded in `source/config/upstream.json`), which comes
from the TotK Recomp lineage. The projects each of them builds on are listed in `source/THIRD_PARTY_NOTICES.md`.

Its authors, by their commits up to the import: Alano6678 (Ksora25), LibertyPrime27, ripps818, fgabe and PrimeWalkerVN,
with the authors of DKB Recomp before them. Their work is the base of everything here.

## Licence

GPL-3.0-or-later, with the MIT parts under `source/LICENSE-MIT`. Every copyright and licence notice of the imported
files stays as it was, in the files themselves, in `source/LICENSE`, `source/LICENSE-MIT` and
`source/THIRD_PARTY_NOTICES.md`. Age of Decomp as a whole is GPL-3.0-or-later for that reason.

## What was left out at the import

Files that belong to the other project as a project, not to the engine: its README, contribution and conduct files,
issue and pull request templates, workflows and hooks, its instructions for coding agents, its installer and that
installer's tests, its build and tester guides, its development log and status notes, and its example settings files.
The technical notes under `source/docs/` were kept.

## From here on

Changes are made in `engine/source/` and committed like any other file. HWAoI Recomp's issues and pull requests are
still read for ideas and for problems that also happen here; a fix taken from there is credited in `docs/CHANGELOG.md`
to its author, with its number.
