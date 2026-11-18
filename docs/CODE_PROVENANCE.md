# Code provenance

Where reused code in Age of Decomp comes from, how it was checked and under which licence it is used. Written
2026-11-18 at repository `5da1dbc5e2d15abbd1173a972077354b056e74ad`, engine pin hwaoi-recomp
`ce035e02f5d77ced2ba967905e45c7226a063656` + `engine/patches/0001`–`0004`, before the in-game menu redesign; the last
section covers what that redesign changes. `THIRD_PARTY_NOTICES.md` holds the short list that ships with releases; the
engine keeps its own complete table in its `THIRD_PARTY_NOTICES.md` (shipped as `engine/source/THIRD_PARTY_NOTICES.md`).

Categories: **identical copy** (bytes equal after line-ending normalisation), **adaptation** (rewritten code that keeps
the source's structure, constants or algorithm), **design similarity** (same idea or workflow, different code),
**vendored dependency** (a third-party component kept in the tree unmodified), **to verify** (a match that this pass
could not settle).

## Method
- `tools/provenance.py compare <ours> <theirs>` (stdlib, in this repository): lines normalised (whitespace collapsed,
  blank and brace-only lines dropped), `difflib.SequenceMatcher` without autojunk; reports matched lines as a share of
  ours and every run of ≥ N equal consecutive lines with line ranges on both sides. `tools/provenance.py tree` does the
  same for same-named files of two trees. Percentages below are "matched lines / our normalised lines".
- Visual constants: every `IM_COL32(...)`/`ImVec4(...)` literal and every `s.<StyleVar> = ...` assignment extracted
  from the menu sources (comments removed) and compared as sets.
- Vendored files: git blob SHA-1 of our file against the GitHub contents API at the cited tag/commit
  (`gh api repos/<owner>/<repo>/contents/<path>?ref=<ref> --jq .sha`), or SHA-256 after CRLF→LF.
- History: `git log -S'<literal>'` in the source repositories to date when a constant first appeared.
- Sources compared (read-only clones in `refs/`, recreated by `tools/fetch_refs.py`; DKB is a shallow clone there, so
  its pinned commit needs `git -C refs/dkb-pc-recomp fetch --depth 1 origin 42fcd9d78b4b44c2831b68ba21e5807d74a96f45`):
  Revivedx/SavageWindRecomp `575ad88e02f9a89a9a885d90039d48af33c982f4` (2026-10-22), peiikai/LASRecomp `da35b83a9611d046a9a681aae668d7085954d37f`
  (2026-10-19), Lucas-blip-png/dkb-pc-recomp `42fcd9d78b4b44c2831b68ba21e5807d74a96f45` (the engine's base), TotK Recomp
  valentderah/totk-pc-recomp `f469c5a7745502357fc1664c04e02522fe92c42a` (0.2.8, the DKB base) and `7a37fc3c346c…`
  (0.4.0; local clone `F:\dev\totk-pc-recomp`, remotes iCosiSenpai fork + valentderah upstream), iCosiSenpai/Deluxe8
  `b6f2594bffe67b0f707316a84c112a99e449e789` (2026-11-15; local `F:\dev\deluxe-8-comp`), elripalda/RecompLauncher
  `df5cd3698171e282b444a2515a12827b1364ae14`, a55uka/nup `a4840f06c94ddeb414eea5b33c9704ffe995f11c`,
  ocornut/imgui tag `v1.91.9` (`97428e8ac99e339ce05eee531cf55b77b29ea709`), google/fonts `main`.
- Artifacts: `out/engine/windows/` and `out/release/AgeOfDecomp-v0.1.0-Windows-x64(.zip)` as built on 2026-11-17
  (listing and SHA-256). The AppImage was not unpacked (needs Linux): its contents are "not verified".

## 1. In-game menu (engine `runtime/ui/`), as pinned

### 1.1 Direct source: TotK Recomp's menu (adaptation, mostly line-for-line)
| Local file (pin) | TotK Recomp 0.2.8 `f469c5a` | DKB `42fcd9d` |
|---|---|---|
| `runtime/ui/option_rows.cpp` (415 lines) | identical | identical |
| `runtime/ui/option_rows.h` (66) | identical | identical |
| `runtime/ui/menu.cpp` (1335) | 94.7 % (1062/1121) | 94.7 % |
| `runtime/ui/menu.h` (76) | 92.5 % | 92.5 % |
| `runtime/ui/binds_page.cpp` (280) | 79.8 % | 79.8 % |
- First written in TotK Recomp by Valiantsin Dzerakh: `c6a7a28` (2026-10-26, "Compile Dear ImGui and draw the menu
  overlay into the swapchain image…") and `f8d584b` (2026-10-26, "Add the tabbed settings menu…"). DKB Recomp took
  them unchanged; HWAoI Recomp changed texts, rows and game hooks.
- Licence: MIT (TotK Recomp `LICENSE`, Copyright (c) 2026 Valiantsin Dzerakh), kept by the engine as `LICENSE-MIT`.
- Attribution present: engine `THIRD_PARTY_NOTICES.md` row "TotK Recomp, the base of this repository … every file
  that came from it" + `LICENSE-MIT`. The file headers name SavageWindRecomp and LAS Recomp, not TotK Recomp.
- In artifacts: `engine/source/runtime/ui/*` in the Windows bundle and zip (menu.cpp SHA-256 `97a8b7749ede…` = pin),
  `engine/source/LICENSE-MIT`, `engine/source/THIRD_PARTY_NOTICES.md`. Compiled into `hwaoi.exe` on the player's PC.

### 1.2 Origin of the look: LAS Recomp → SavageWindRecomp (adaptation of constants and drawing, rewritten code)
- Local: `runtime/ui/menu.cpp:197-218` (`style_init`), `:922-994` (`layout_menu`: window over a dimmed game, gold
  title, tab bar), `runtime/ui/option_rows.cpp:112-198` (`row`: fill/outline, radius 9, padding 16, gold 2 px focus
  border, drawn ◀ ▶ triangles 0.42/0.55 em), `:69-94` (two panes, gold divider).
- Sources: SavageWindRecomp `runtime/ui/menu.mm:312-348` (`style_init`), `:1005-1035`, `runtime/ui/option_rows.mm:52-133`
  (`row`); LAS Recomp `runtime/ui/menu.mm:286-322`, `runtime/ui/option_rows.mm:50-…`. The constants first appear in
  LAS Recomp `f742666` (2026-10-13, "LARecomp Beta 3", Peiikai); SavageWindRecomp has them from its first public
  commit `0efcef9` (2026-10-16) and is built on LAS Recomp.
- Comparison: lines 4.1 % (menu.cpp vs SW menu.mm; 2.3 % vs LAS), 8.1 % (option_rows.cpp vs both), 7.0 %
  (option_rows.h). Constants: 23 of our 31 distinct colour literals and 9 of 10 style metrics are identical to both
  SW and LAS (WindowRounding 14, FrameRounding 8, GrabRounding 8, TabRounding 8, ChildRounding 10, FramePadding
  14×8, ItemSpacing 14×12, ScrollbarSize 18, WindowBorderSize 0; WindowBg 0.06/0.09/0.08/0.94, gold 0.93/0.76/0.33,
  greens 0.12–0.21, text 0.96/0.94/0.88, row fills α22/α40, gold 236/194/84 borders, dim α120).
- The maintainer's observation "the look is 100 % the source repository's" — **partly confirmed**: palette, row
  drawing, stepper arrows, gold focus border, tab colours and the dimmed full-screen backdrop are the same; it is not a
  pixel copy: Roboto Medium instead of SW's Fira Sans, sentence case instead of SW's upper-case labels, 2.2 em
  two-line rows with notes instead of 1.9 em, rows/description split 58/42 instead of 48/52, window 0.88 w × 0.86 h
  capped at 1400 × 820 instead of 0.9 w × full height capped at 1500, padding 28×22 instead of 28×24, no close ✕.
  SW and LAS are macOS/Metal-first and were not rendered here, so no pixel comparison was made.
- Licence: MIT. SavageWindRecomp `LICENSE`: Copyright (c) 2026 Revivedx and the Savage Wind Recomp contributors,
  Copyright (c) 2026 Peiikai (LAS Recomp, on which this project is built). LAS Recomp: Copyright (c) 2026 Peiikai.
- Attribution present: `menu.cpp:9-12` and `option_rows.cpp:3-7` name SavageWindRecomp at `575ad88…` with both
  copyright lines; engine notices row "SavageWindRecomp settings menu". LAS Recomp is credited only for the Vulkan
  set-up, not as the first source of the look (covered by the Peiikai copyright line of SW's notice).

### 1.3 Vulkan set-up of Dear ImGui (adaptation)
- Local `runtime/ui/menu.cpp:239-266` (`vk_up`); source LAS Recomp `runtime/nvn/vulkan/imgui_glue.cpp:12-33`
  (`vk_imgui_backend_init`) at `da35b83…`. Same `ImGui_ImplVulkan_InitInfo` field sequence (API 1.3, dynamic
  rendering into the swapchain format, internal pool, MinImageCount 2), different values (pool 16 vs 64, ImageCount 4 vs
  swapchain count + 1, no pipeline cache). The sequence is also the stock Dear ImGui example usage.
- Licence MIT, Copyright (c) 2026 Peiikai. Attribution present: `menu.cpp:5-7`; engine notices (LAS host runtime row,
  "in-game menu: runtime/ui/menu.cpp").

### 1.4 Performance overlay (adaptation)
- Local `runtime/ui/menu.cpp:997-1243` (fps line and full panel, `perf_full_draw` at :1150); source SavageWindRecomp
  `runtime/ui/menu.mm:736-778` (`perf_cell`, `perf_overlay_draw`): top-right window, background α0.6, the same window
  flags, GPU/VRAM/CPU/RAM/FPS table cells with "-" for missing numbers, a frame-time plot. Ours adds the picture
  lines, translated formats and fixed column widths. Licence MIT (both copyright lines). Attribution present:
  `menu.cpp:997-1000` and the engine notices rows for the panel and `runtime/perfmon.c`.

### 1.5 Vendored by the menu (identical copies)
| Local | Source | Method | Licence | Attribution |
|---|---|---|---|---|
| `runtime/third_party/imgui/` (13 files incl. `backends/imgui_impl_vulkan.*`) | ocornut/imgui `v1.91.9`, via LAS Recomp `third_party/imgui/` | git blob SHA-1 equal to the tag for every file; SHA-256 equal to LAS's copy | MIT, Copyright (c) 2014-2025 Omar Cornut | `LICENSE.txt` kept; engine notices row; our notices row |
| `runtime/ui/fonts/Roboto-Medium.ttf` | imgui `v1.91.9` `misc/fonts/` (Google Roboto), via LAS Recomp | blob SHA-1 equal | Apache-2.0 | `fonts/LICENSE-Roboto.txt`; engine notices row |
All are in the Windows bundle and zip (`engine/source/runtime/...`, licence files included).

## 2. Launcher (`apps/launcher/`)

### 2.1 From Deluxe8, the maintainer's own launcher (adaptation)
Deluxe8 (iCosiSenpai/Deluxe8 `b6f2594…`, GPL-3.0-or-later, public; files first committed 2026-11-15 in `8661dd0` and
`e207588`, a day before Age of Decomp's first commit `c4b9e68`):
| Local | Deluxe8 | Matched | Longest run |
|---|---|---|---|
| `src/lib/nav.ts` | `apps/launcher/src/lib/nav.ts` | 55.6 % | 35 lines (ours 50-90 ↔ 49-89), 17 lines (13-35 ↔ 10-32) |
| `src/lib/i18n.svelte.ts` | same path | 68.8 % | 6 |
| `scripts/check-i18n.mjs` | same path | 50.0 % | 9 (33-42 ↔ 37-46) |
| `src/lib/components/TitleBar.svelte` | same path | 46.6 % | 4 |
| `Segmented.svelte`, `Toasts.svelte`, `Toggle.svelte`, `Slider.svelte` | same paths | 37–41 % | 4–7 |
| `src/App.svelte`, `src/styles/base.css` | same paths | 28 %, 22 % | 7, 5 |
| `src/main.ts` | same path | identical (6-line Svelte mount) | — |
Rust backend files of the same names (`lib.rs`, `paths.rs`, `prepare.rs`, `game.rs`, `settings.rs`, `system.rs`):
0–12 %, no run longer than 4 lines — written anew. Licence: GPL-3.0-or-later (same author, same licence as ours).
Attribution present: `THIRD_PARTY_NOTICES.md` row "Launcher navigation, i18n pattern and engine-pin workflow" (now
with the commit). In artifacts: compiled into `AgeOfDecomp.exe`.

### 2.2 RecompLauncher (design reference only, no copy found)
Launcher Rust vs RecompLauncher `src-tauri/src/*.rs` (`df5cd36…`, GPL-3.0): at most 11 % matched lines, all generic
(`use` lines, test attributes), no run over 4 lines; `crates/aod-nup` vs its `nup.rs`: 2.1 %. The ini line-editing
behaviour follows RecompLauncher's documented contract (design similarity). `tools/recomplauncher_check.rs` is our
harness appended to a scratch copy of RecompLauncher's `devtools.rs` at test time; it calls RecompLauncher's own
functions and is never shipped.

### 2.3 Fonts (vendored, identical)
`src/assets/fonts/Inter-Variable.ttf` = google/fonts `ofl/inter/Inter[opsz,wght].ttf` (blob `047c92f6…`, last changed
`e1d6480`, 2024-06-06); `Cinzel-Variable.ttf` = google/fonts `ofl/cinzel/Cinzel[wght].ttf` = NDISCOVER/Cinzel
`fonts/variable/Cinzel[wght].ttf` (blob `d218a0b9…`). SIL OFL 1.1; `OFL-Inter.txt`, `OFL-Cinzel.txt` alongside and in
the release `licenses/`.

## 3. NUP reader (`crates/aod-nup/`) — own implementation
Written from `refs/nup/SPEC.md` (a55uka/nup, MIT): 0 % matched lines with the project's Python tools
(`tools/nup/nup/*.py`), 2.1 % with RecompLauncher's `nup.rs`. Attribution present ("own implementation" row).

## 4. Tools
- `tools/engine.py` toolchain pins — **adaptation of data and pattern** from TotK Recomp (valentderah/totk-pc-recomp,
  MIT, Copyright (c) 2026 Valiantsin Dzerakh): 10 of our 15 `PINS` (URL + SHA-256) are the ones in its
  `tools/make_release.py` / `make_release_linux.py` — llvm-mingw 20261020, Ninja 1.13.2, MSYS2 opus, libvpx, zstd,
  vulkan-headers, vulkan-loader, vma, AppImage runtime 20251108 (added in `1ece856`, 2026-10-24) and the SDL 3.4.20
  devel package (`2fe1ea1`, 2026-11-10, "Bump the bundled Windows SDL to 3.4.20 (#912)"); nine of them are also in the
  hwaoi-recomp and DKB `make_release.py`. The `PINS` table + `fetch()`/`unpack()` with a stamp file follow
  `make_release.py` (7.2 % matched lines, runs of 3–5 lines). Ours: Python 3.14.9 NuGet, python-build-standalone
  20261110, zlib, libusb, SDL source. The notices named this "the maintainer's own totk-pc-recomp"
  (`github.com/iCosiSenpai/totk-pc-recomp`), which returns 404 to the GitHub API even for the owner; the lines were
  written by Valiantsin Dzerakh upstream — notices corrected.
- Engine pin workflow (`engine/upstream.json` + `engine/patches/` + branch + `export-patches`) — **design similarity**
  with Deluxe8 `tools/engine.ps1` (PowerShell vs Python, no shared code).
- `tools/check_content.py` vs the engine's: 16 % matched (common imports and the idea), written anew.
- `tools/release.py` vs the engine's `make_release_linux.py`: 3.7 %, own.

## 5. Engine patch queue (`engine/patches/0001`–`0004`) — own code
0001 NuGet cache, 0002 Linux build in `compiler/backend.py`, 0003 `tools/hostmem.py`, 0004 SDL HIDAPI hints
(`runtime/input.c`; the hint list is the same as the launcher's `gamepad.rs:213-223`, both ours).

## 6. Inherited engine components (pointer, not re-verified here)
The engine's `THIRD_PARTY_NOTICES.md` lists every other source with commit and licence: TotK Recomp (base, MIT), LAS
Recomp (lifter, generator, host runtime, Vulkan code; MIT), MacNativeGames (host runtime, audio, RomFS, patches; MIT),
d2r-mobile / nxnative (MIT), deko3d (zlib), Ryujinx shader translator 1.3.2 and audio (MIT, via MacNativeGames),
Eglantine Ampere lowering (GPL-3.0-or-later, from Franseven/PokeDinner-a-Eglantine-Fork `1dd40d86…` on
git.eden-emu.dev, cross-checked with Djihads80/Eglantine `8b8d0fc…`), AMD FSR 1 and ADL (MIT), VMA (MIT), SDL, Opus,
libvpx, Zstandard. **To verify** in a later pass: byte comparisons of those rows against their sources (only the
Dear ImGui and Roboto rows were checked here). Our `THIRD_PARTY_NOTICES.md` had the Eglantine source (a different
repository), its licence ("GPL-3.0") and the TotK lineage licence ("GPL-3.0 / MIT") wrong; corrected.

## 7. After the in-game menu redesign (engine patches 0005–0007, 2026-11-18)
`0005` adds `tests/menu_preview*` (own code: a headless Vulkan host and a stdlib driver). `0006` is the redesign and
`0007` the Italian menu language (own translations, launcher glossary). The file headers and the engine
`THIRD_PARTY_NOTICES.md` rows changed by `0006` say what remains adapted:
- The look and layout (rail, cards, rows, controls, detail strip, action bar, modals, theme tokens) are new code
  following the launcher's design ("Seal of the Imprisoned"); the SavageWindRecomp/LAS constants of 1.2 are gone.
- Still adapted and still credited in the file headers: TotK Recomp's menu structure (event queue, scale, test hooks,
  option-row API, tabs' rows) (1.1), LAS Recomp's Vulkan set-up (1.3), SavageWindRecomp's performance overlay (1.4),
  page switching with L / R and Q / E and the rows' left / right stepping with the hold speed-up (from
  SavageWindRecomp's `option_rows.mm`).
- Measured at engine `41bfc52` (same method as 1.2): colour literals shared with SavageWindRecomp and LAS Recomp:
  1 of 23 (transparent black); style metrics: 1 of 11 (FramePadding 14×8). Lines: `menu.cpp` 1.4 % vs SW `menu.mm`,
  77.7 % vs the pinned `menu.cpp` (pages, overlay, test hooks and Vulkan code stay); `option_rows.cpp` 5.0 % vs SW,
  25.0 % vs the pinned file (the save logic, focus handling and restored comments).
- New vendored files: `runtime/ui/fonts/Inter-Variable.ttf`, `Cinzel-Variable.ttf`, `OFL-Inter.txt`, `OFL-Cinzel.txt`
  — byte copies of the launcher's (2.3), embedded by `runtime/ui/menu.cpp`. Roboto stays in the tree, no longer used.
