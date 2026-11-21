# Age of Decomp engine

The compiler and the runtime behind [Age of Decomp](../../README.md), the PC version of the player's own copy of
*Hyrule Warriors: Age of Imprisonment* (Nintendo Switch 2, title `04003F40232DE000`, update NCM `196608`).

- `compiler/`: the build a player's PC runs (`backend.py`): check the copy, extract the program code, prepare the
  resources, translate ARM64 to C, compile and link, set up the shader translator.
- `recomp/`: the ARM64 to C translator.
- `runtime/`: what the translated game links against: Horizon services, NVN2 on Vulkan, shaders to SPIR-V, input
  through SDL 3, audio, the settings menu (`runtime/ui/`).
- `tools/`: the generator's scripts and the shader translator's sources (`tools/vendor/ryujinx`).
- `config/games/`: the game profile: the module hashes a copy must have.
- `tests/`: tests that need no game, among them the menu on a synthetic scene (`tests/menu_preview_test.py`).
- `docs/`: technical notes.

Nothing of the game is here, and nothing built from the game may be committed: see `.gitignore` and the repository's
`tools/check_content.py`.

Build it through the repository's `tools/engine.py` (`toolchain`, `translator`, `stage`, `game`); `AGENTS.md` at the
repository root has the workflow.

Where this code comes from, who wrote it and under which licence: [`../ORIGIN.md`](../ORIGIN.md), `LICENSE`,
`LICENSE-MIT` and `THIRD_PARTY_NOTICES.md`. It is GPL-3.0-or-later.
