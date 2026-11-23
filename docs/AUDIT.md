# Audit of the build, setup and launch flows (2026-11-18)

> Since 2026-11-21 the engine is in `engine/source` (see `engine/ORIGIN.md`), no longer a pin plus
> `engine/patches`. The paths and the workflow below are the ones at the time of this audit.

Scope: Age of Decomp at `5da1dbc5e2d15abbd1173a972077354b056e74ad` (launcher, `aod-cli`, `crates/aod-nup`, `tools/`,
`packaging/`, `.github/`) and the engine as the project builds it: hwaoi-recomp `ce035e02f5d77ced2ba967905e45c7226a063656`
with `engine/patches/0001`–`0004`. Windows and Linux paths were both read; Windows is where things were executed.
The in-game menu redesign of the same session is a separate engine patch. The problems below are outside it: they are
documented here with a proposed fix and are **not** changed by that patch, so its diff stays reviewable.

Method: the GUI → Rust → Python → engine paths were followed in the source. The confirmed findings were reproduced
with synthetic NUP files written by a small stdlib script (`work/audit/make_bad_nup.py`, in Appendix A) and with
temporary Rust tests (Appendix B), deleted after they ran. No game file, dump, key or firmware was used or looked for.

Severity is the impact on a player or on the project if the finding is hit: **High** = data loss or code execution
without unusual prerequisites; **Medium** = data loss, a stuck or broken flow, or non-reproducible releases under
plausible conditions; **Low** = needs unusual prerequisites, or only hardening. Confidence: **high** = reproduced, or
a direct reading of a short code path; **medium** = code reading with a timing or environment assumption.

## Follow-up on 2026-11-18

The findings below describe the starting revision and retain its original file references. The status table records
subsequent fixes in 69523de, ba5c717, e62e5a3 and bebc5fa. Verification includes 7 NUP tests, 18 launcher/CLI tests,
4 release-guard tests, clippy, and the huge/DAG/cycle synthetic NUP regressions. The updater added four regression
tests, taking launcher/CLI tests to 23 (including the later config-save guard). Read partial and mitigated entries together with these limits.

A-17 remains a hypothesis. The runtime's `cfg_save_to` rereads the current file and merges dirty keys, reducing
ordinary sequential overwrites. Simultaneous read-modify-write operations still have no shared interprocess lock.
A-02 handles GUI close and Windows process death; killing the Linux CLI with SIGKILL can leave grandchildren.
A-18 backs up bad configuration before defaults are used. A save guard now refuses to overwrite an invalid or
unreadable file even when its backup rename failed. NuGet dependency locks, real Linux GPU/Steam Deck and the remaining physical controller matrix are open.
The final menu patch series clean-applied with tree 680fe17205f4bbfbc684ef76a501b37ed37b250c, matching the local
engine. The latest real-game run exited 0 and produced six menu captures; the Italian Screen and Menu pages were
visually inspected, including the corrected two-line version label. Player-game captures stay in ignored work/.

## Summary

| ID | Finding | Severity | Confidence | Status |
|---|---|---|---|---|
| A-01 | A `hwaoi.ini` that cannot be read as UTF-8 is treated as empty: the next launcher write erases every other setting | Medium | high | Fixed — 69523de |
| A-02 | Closing the launcher during setup leaves the engine build running with nothing to show or stop it | Medium | high | Partial — ba5c717; Linux SIGKILL limitation remains |
| A-03 | `start_prepare` stays "busy" for good when the job cannot be built from the config | Medium | high | Fixed — ba5c717 |
| A-04 | `engine.py stage` ships the engine's working tree and labels it with `HEAD` | Medium | high | Fixed — bebc5fa |
| A-05 | CI never checks that `engine/patches` apply to the pin, nor any engine code | Medium | high | Partial — patch application checked in CI; runtime builds remain local |
| A-06 | A NUP entry can declare any decompressed size: `read_all` aborts the process on the allocation | Low | high | Fixed — e62e5a3 |
| A-07 | NUP folders can share sub-folders: a 2.8 KB file lists 8,000,000 files | Low | high | Fixed — e62e5a3 |
| A-08 | Setting values with line breaks pass validation and inject `hwaoi.ini` lines | Low | high | Fixed — 69523de |
| A-09 | `packaging/recomplauncher.json` and `docs/SETTINGS.md` can drift from the catalogue unnoticed | Low | high | Fixed — regenerated files and CI comparison |
| A-10 | Release workflow: actions pinned by tag in a `contents: write` job; Linux AppImage built only locally | Low | high | Partial — actions pinned and release permissions split; Linux artifact remains local |
| A-11 | Cancel racing the end of a build reports a finished build as cancelled | Low | medium | Fixed — ba5c717 |
| A-12 | Launch check ignores the identity of a separate update NUP | Low | high | Fixed — ba5c717 |
| A-13 | `aod-cli --jobs` accepts values the engine refuses (exit 4 instead of 2) | Low | high | Fixed — ba5c717 |
| A-14 | `aod-ready.json` survives a later failed or cancelled prepare | Low | high | Fixed — ba5c717 |
| A-15 | Bare-name `SDL3.dll` fallback uses the default DLL search order | Low | high | Fixed for SDL fallback — ba5c717; global DLL-search hardening remains optional |
| A-16 | The developer engine search (`out/engine/<os>` in 8 parent folders) is active in release builds | Low | high | Mitigated — development search requires repository markers |
| A-17 | Three writers of `hwaoi.ini` (launcher, RecompLauncher, the running game) without coordination | Low | medium | Open hypothesis — not reproduced; runtime merges dirty keys |
| A-18 | An unreadable `launcher.json` silently becomes the defaults and is overwritten on the next save | Low | high | Fixed — backups plus save guard on unpreserved bad config |
| A-19 | PyPI and NuGet packages pinned by version only (no hashes, no lock file) | Low | high | Partial — hashed Python wheels; NuGet restore lock remains open |

Checked and refuted: see "Hypotheses that did not hold" at the end.

## Findings

### A-01 — unreadable `hwaoi.ini` is replaced by a one-line file (Medium, high, confirmed)
- Prerequisites: `hwaoi.ini` (or the pending file) contains bytes that are not UTF-8 — e.g. the player edited a comment
  in Notepad with an ANSI code page (`è` = 0xE8 in Windows-1252) — or the read fails for another reason (sharing
  violation, permissions).
- Path: Settings page → `set_setting` (`apps/launcher/src-tauri/src/lib.rs:169-174`) → `settings::write`
  (`settings.rs:76-82`) → `Ini::load` (`ini.rs:51-55`: `read_to_string(...).unwrap_or_default()`) → `set` → `save`.
  The same load-then-save is in `settings::merge_pending` (`settings.rs:101-118`) and `prepare::stash_early_settings`
  (`prepare.rs:147-153`), and `reset` (`settings.rs:84-98`).
- Impact: every other setting, comment and unknown key in the file is lost; the game falls back to its defaults.
- Evidence: Appendix B test 1 wrote a file with one Windows-1252 byte, `[display] output_res = 2560x1440` and
  `[audio] volume = 150`, then set `motion = 0`: the file afterwards held only `[input]\nmotion = 0`.
- Fix: `Ini::load` returns `io::Result<Ini>`; `NotFound` gives an empty file, any other error is returned and the
  write is refused with a message ("hwaoi.ini could not be read: … — nothing was changed"). For non-UTF-8 content,
  either refuse the same way or keep the lines as bytes (`Vec<Vec<u8>>`) so untouched lines are written back unchanged.
- Verify: turn Appendix B test 1 into a unit test in `ini.rs` that expects an error (or unchanged foreign lines).

### A-02 — closing the launcher during setup orphans the build (Medium, high, confirmed by code)
- Prerequisites: a build is running (stages 2–6 take minutes) and the player closes the window (title-bar ✕,
  `TitleBar.svelte:18`), or the launcher is killed. Same for `aod-cli prepare` killed by its caller.
- Path: `prepare::run` spawns `python … backend.py build` (`prepare.rs:182-208`); nothing handles
  `CloseRequested`/exit (no `on_window_event` or `RunEvent` handler in `lib.rs`), and Windows does not end child
  processes with their parent unless they are in a job object. On Linux the build is in its own process group
  (`prepare.rs:202-206`), which also outlives the launcher.
- Impact: Python, Ninja and clang keep compiling with no window; the engine's workspace lock stays held, so the next
  Setup fails with "This output folder is already being compiled" (`compiler/cache.py:51-70`) until the orphan ends.
- Fix: while `preparing`, intercept `CloseRequested` and ask "Stop the build and close?" → `prepare::cancel`, wait for
  the child, then exit. Windows: create the child inside a job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (covers
  crashes and `aod-cli` being killed). Linux: `prctl(PR_SET_PDEATHSIG, SIGTERM)` in `pre_exec`, plus a group kill on
  exit.
- Verify: start a build in the GUI, close the window during stage 5, then `tasklist | findstr /i "python clang ninja"`
  (Windows) or `pgrep -a clang` (Linux): today the processes are still there.

### A-03 — `start_prepare` stays busy after an early error (Medium, high, confirmed by code)
- Path: `lib.rs:130-141` sets `preparing = true`, then `prepare::Job::from_config(...)?` can return early (no NUP in
  the config: "Choose your game file first", `prepare.rs:63-73`) without resetting the flag.
- Impact: every later "Build" answers "A build is already running" until the launcher restarts.
- Fix: build the `Job` before taking the flag, or reset it on every early return (a small guard type).
- Verify: with an empty `launcher.json`, invoke `start_prepare` twice from the devtools console
  (`window.__TAURI__.core.invoke('start_prepare')`): the second error is "already running".

### A-04 — the engine bundle is staged from the working tree (Medium, high, confirmed by code)
- Path: `tools/engine.py` `source_files()` lists tracked names (`:473-479`) but `cmd_stage` copies them from the
  working tree (`:511-515`) and records `source_revision = HEAD` (`:530`). `cmd_stage` never checks
  `git status`. `tools/release.py windows|linux` packs whatever is in `out/engine/<os>` and the repository's working
  tree (catalogue, docs) the same way.
- Impact: a bundle or release can carry uncommitted engine edits that `engine/patches` does not reproduce, while
  untracked new files are silently left out (a build on the player's PC would then miss them). On 2026-11-18 the engine
  checkout held exactly such uncommitted edits (an unfinished Italian menu language), so a `stage` that day would have
  shipped them.
- Fix: refuse `stage` when `git status --porcelain` (outside `work/`) is not empty, or stage from
  `git archive HEAD`; write `"dirty": true` and the diff's SHA-256 into `bundle.json` when a developer forces it.
  Stamp releases with `git describe --always --dirty`.
- Verify: edit a tracked runtime file, `py -I tools/engine.py stage`, then compare
  `out/engine/windows/source/<file>` with `git -C .cache/engine/hwaoi-recomp show HEAD:<file>`.

### A-05 — no CI coverage of the engine patch queue (Medium, high, confirmed by code)
- Path: `.github/workflows/ci.yml` runs the content guard, the NUP crate and the launcher only.
- Impact: a patch that no longer applies to the pin, or engine code that no longer compiles, is found only when a
  developer runs `fetch`/`game` locally; the release workflow is the first automated place it would fail.
- Fix: an Ubuntu job: `AOD_CACHE_DIR=$RUNNER_TEMP/aod python3 tools/engine.py fetch` (clone + `git am` of every patch),
  then the engine's own game-free tests, and the menu preview harness (`tests/menu_preview_test.py`, added by the menu
  patch) on Mesa's lavapipe (`mesa-vulkan-drivers`).
- Verify: introduce a conflicting hunk in a patch: today CI stays green.

### A-06 — NUP entry size is trusted for the allocation (Low, high, confirmed by PoC)
- Prerequisites: the player selects a crafted or badly damaged `.nup` (drag and drop, "Choose file", RecompLauncher).
- Path: `inspect_nup` (`lib.rs:105-114`) / `aod-cli verify|prepare` / `aod-nup check` → `profile::check`
  (`crates/aod-nup/src/profile.rs:151-163`) → `Nup::read_all` (`crates/aod-nup/src/lib.rs:426-449`):
  `Vec::with_capacity(e.size as usize)` with `size` from the TOC, and `read_to_end` on the zstd decoder without an
  output cap. The TOC SHA-256 is checked, but the file carries its own TOC hash, and the module hashes the check
  compares against are public (`engine/profile/*.json`).
- Impact: Rust aborts the whole process on a failed allocation — in the GUI the launcher window disappears; with a real
  zstd stream, memory grows without bound before the size check.
- Evidence: `huge.nup` (409 bytes, Appendix A: an update package whose `main.npdm` declares the profile hash, zstd,
  size 2^40) → `aod-nup check huge.nup` printed `memory allocation of 1099511627776 bytes failed` and aborted.
- Fix: refuse entries whose `size` is above a bound for their use (ExeFS modules: 512 MiB) before reading; reserve
  `min(size, 64 MiB)`; decode through `.take(size + 1)` and fail when more comes out.
- Verify: `py -I work/audit/make_bad_nup.py huge huge.nup` then `aod-nup check huge.nup` → expected "damaged" (exit 2).

### A-07 — folder sharing amplifies a tiny NUP (Low, high, confirmed by PoC)
- Path: `Nup::walk` (`lib.rs:383-404`) follows `children` (`lib.rs:343-367`) without remembering visited folders;
  only depth is limited (64). Reached by `profile::check` (ExeFS and RomFS walks, `profile.rs:143`, `:177`) and
  `aod-nup info` (`romfs_stats`).
- Impact: CPU and memory grow as fan-out^depth: a stuck "Checks" step or an out-of-memory abort.
- Evidence (Appendix A `dag<N>`: root → N × dir1 → N × dir2 → N × dir3 → 1 file): 1025 B → 125,000 files listed in
  0.07 s; 1633 B → 1,000,000 in 0.27 s; 2225 B → 3,375,000 in 0.88 s; 2833 B → 8,000,000 in 2.05 s (`aod-nup info`,
  release build). A cycle is caught quickly by the depth limit (see refuted R-2).
- Fix: a visited bit per folder (a NUP TOC is a tree: a folder seen twice is corrupt), and stop when the number of
  listed files exceeds `file_count`.
- Verify: the `dag200` file must fail with "damaged" in milliseconds.

### A-08 — setting values can carry line breaks (Low, high, confirmed by test)
- Prerequisites: a value reaching `set_setting` that the UI would never send (the webview is local, CSP-restricted
  content, so this is defence in depth against a future bug or an injected script).
- Path: `settings::validate` (`settings.rs:43-62`): a `pads` value only needs `pad:` and two `:`-separated parts; a
  `slider` value is any `f64` text (`1e2`); `Ini::set` (`ini.rs:74-81`) writes it verbatim.
- Evidence: Appendix B test 2: `pad:00ff\nfps_limit = 120:1` was accepted and read back as two keys
  (`player1_device = pad:00ff`, `fps_limit = 120:1`).
- Fix: reject control characters, `=`, `#`, `;`, `[` in every value; `pad:` + 32 hex digits + `:` + decimal index;
  sliders as plain decimal numbers.
- Verify: the same test must get `Err`.

### A-09 — generated manifest and docs are not checked (Low, high, confirmed)
- Path: `packaging/recomplauncher.json` and `docs/SETTINGS.md` are written by `tools/release.py manifest|docs`
  (`release.py:47-150`) from `apps/launcher/src/lib/settings/catalog.json`; CI does not compare them.
- Evidence: with the uncommitted catalogue entry `menu_lang` present on 2026-11-18, `release.manifest()` differs from
  the tracked `packaging/recomplauncher.json` (`menu_lang` missing from the tracked file).
- Fix: in CI, regenerate both and `git diff --exit-code packaging/recomplauncher.json docs/SETTINGS.md`.
- Verify: `py -I -c "import sys,json; sys.path.insert(0,'tools'); import release; print(json.dumps(release.manifest(), indent=2, ensure_ascii=False)+'\n' == open('packaging/recomplauncher.json', encoding='utf-8').read())"`.

### A-10 — release workflow hardening (Low, high, confirmed by code)
- Path: `.github/workflows/release.yml:7-30`: `permissions: contents: write` for the whole job, every action by a
  mutable tag (`actions/checkout@v5`, `astral-sh/setup-uv@v6`, `dtolnay/rust-toolchain@stable`, …),
  `workflow_dispatch` on any ref; only the Windows zip is built, the AppImage exists only as local builds.
- Fix: pin actions by commit SHA (tag in a comment), build in a read-only job and upload in a small `contents: write`
  job, run dispatch only for `refs/tags/v*`; add an Ubuntu job for `release.py linux --appimage`.

### A-11 — cancel racing completion (Low, medium, confirmed by code)
- Path: `prepare::run` checks `control.cancel` (`prepare.rs:263-266`) before `complete && status.success()`; `cancel`
  sets the flag whenever `pid != 0` (`prepare.rs:289-305`). A click after Python wrote `BUILD COMPLETE` but before
  `pid` is reset reports "cancelled": no `aod-ready.json`, no `merge_pending`.
- Fix: decide on `status.success() && complete` first; ignore a cancel that arrives after the child exited.

### A-12 — the separate update NUP is not part of the launch check (Low, high, confirmed by code)
- Path: `game::check` (`game.rs:12-29`) compares header/TOC/size of the main NUP only; the engine records
  `nup_identity.update` and its own `launch` compares the whole identity (`compiler/backend.py:419`).
- Impact: a moved or replaced update NUP is not reported as exit 65 (RecompLauncher's message); the update's files are
  copied into the resource cache during the build, so play usually still works — the behaviour differs from the engine.
- Fix: open `state.update_nup` when present and compare `nup_identity.update`.

### A-13 — `aod-cli --jobs` range (Low, high, confirmed by code)
- `aod-cli.rs:91` passes any number; the engine requires `0 <= jobs <= 8` (`backend.py:229`) and fails with
  "Invalid build settings", reported as exit 4 "build failed". Clamp to 0..8 (as the GUI does, `prepare.rs:69`) or
  exit 2.

### A-14 — stale `aod-ready.json` (Low, high, confirmed by code)
- `aod-cli.rs:120-123` writes the marker on success and nothing removes it. After a failed re-prepare (for example
  after a `prepare.version` bump) RecompLauncher still sees the done marker; Play then exits 64. Remove the marker
  when `prepare` starts and write it atomically (`.part` + rename).

### A-15 — bare-name SDL fallback (Low, high, confirmed by code)
- `gamepad.rs:435-445` ends the candidate list with `SDL3.dll` by name; `Library::new` (`gamepad.rs:159-163`) then uses
  the default search order (application folder, system folders, current folder, `PATH`). Only reached when the
  engine's own copy is missing (development or damaged install), so the risk is low. The `libusb-1.0.dll` that SDL
  opens by name is already preloaded by full path (`gamepad.rs:150-160`), see R-1.
- Fix (Windows): load only absolute paths, and call `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)` at
  start-up so later name-only loads (e.g. `ash::Entry::load()`, `system.rs:49`) skip the current folder.

### A-16 — development engine search in release builds (Low, high, confirmed by code)
- `paths.rs:61-74` tries `<ancestor>/out/engine/<os>` for 8 parent folders of the executable, in every build. A
  release copied without its `engine/` folder could pick up an unrelated `out/engine/windows` and run its Python.
  Limit that search to `cfg(debug_assertions)` or to `AOD_ENGINE_DIR`.

### A-17 — concurrent `hwaoi.ini` writers (Low, medium, hypothesis)
- The launcher (`ini.rs:119-129`, read-modify-write), RecompLauncher (writes on every change) and the running game's
  in-game menu (`runtime/cfg.c` `cfg_save_to`) each rewrite the file without a lock; a change made in the launcher
  while the game runs can be overwritten by the game's next save, and the reverse. Not reproduced.
- Fix: the launcher refuses settings writes while `game_running`, or compares the file's mtime before writing and
  re-reads on a change.

### A-18 — `launcher.json` errors are silent (Low, high, confirmed by code)
- `config.rs:33-35` turns any read or parse error into the defaults; the next `save` (`config.rs:37-43`) overwrites the
  player's file (game path, output folder). Keep the bad file as `launcher.json.bad` and tell the player.

### A-19 — package pins without hashes (Low, high, confirmed by code)
- Python wheels: `tools/engine.py:277-281` (Windows), `:388-389` (Linux) install `numpy==2.5.4 lz4==4.4.5
  zstandard==0.25.0 capstone==5.0.7` by version only. The translator's NuGet restore (`engine.py` passes `--online`
  to upstream `tools/build_xlate_hwaoi.py:264-270`) has no `packages.lock.json`. Everything else downloaded by
  `engine.py` is pinned by SHA-256 (`PINS`) and verified before unpacking.
- Fix: a requirements file with `--hash=sha256:` lines and `--require-hashes`; NuGet lock file with
  `RestoreLockedMode`.

## Areas reviewed without findings
- NUP superblock/TOC parsing: file size, TOC size (≤ 256 MiB), TOC hash, table bounds, package folder indices, meta
  block (≤ 16 MiB) and name bounds are all checked (`crates/aod-nup/src/lib.rs:154-238`, `:302-340`).
- Base/Update/DLC selection mirrors the engine (`profile.rs:63-77`); the engine re-validates title, NCM version,
  module hashes and RomFS inventory, and refuses an output that contains any input NUP (`backend.py:161-176`).
- Engine output folder: per-folder OS lock (msvcrt/flock), identity re-checked after the lock, state written
  atomically, the game executable must not be running (`backend.py:243-275`, `compiler/cache.py:51-76`).
- Launcher IPC: CSP `default-src 'self'`, capabilities limited to window controls and the open dialog
  (`capabilities/default.json`); folders are opened by kind, never by a path from the webview (`lib.rs:242-259`).
- Python is started with `-E -s` and `PYTHONNOUSERSITE` (`prepare.rs:183`, `:195`); engine output is read as bytes.
- Toolchain downloads: SHA-256 before use, resumable `.part`, `tarfile` with `filter="data"`, zip members sanitised by
  `zipfile` (`engine.py:140-190`).
- Launch environment: absolute `HWAOI_ZLIB_DLL`, the translator and the NUP identity checked before start
  (`game.rs:36-113`).

## Hypotheses that did not hold
- R-1 *SDL could load a `libusb-1.0.dll` from the current folder in the launcher*: no — the launcher loads the engine's
  copy by full path before SDL asks for it by name (`gamepad.rs:150-160`).
- R-2 *A folder that lists itself makes `walk` loop for ever*: no — the depth-first walk reaches depth 65 at once and
  returns "folder tree too deep" (`cycle.nup`, Appendix A, answered in milliseconds). Sharing without a cycle is A-07.
- R-3 *RecompLauncher cannot find `AgeOfDecomp-x86_64.AppImage` because the asset is versioned*: no — RecompLauncher
  rebinds `.AppImage` program entries to the installed AppImage (`refs/RecompLauncher/docs/DEVELOPERS.md:96-111`).

## Already tracked, not repeated here
Real Linux / Steam Deck tests and kernel ≥ 6.7 write watch (X-4), portable Linux base and bundled toolchain (X-5),
our own `libSDL3.so.0` (X-6), the real RecompLauncher app (R-3b), the controller matrix (C-5): `plan&activity/TASKS.md`.

## Appendix A — synthetic NUP generator
`work/audit/make_bad_nup.py` (stdlib only; layout of `refs/nup/SPEC.md`, as in `crates/aod-nup/src/lib.rs`
`tests::synthetic`). Usage: `py -I make_bad_nup.py cycle|huge|dag<N> <out.nup>`; then
`crates/aod-nup/target/release/aod-nup.exe info|check <out.nup>`. The files contain no game data.

## Appendix B — temporary tests (run with `cargo test --test <name> -- --nocapture` in `apps/launcher/src-tauri`)
1. `non_utf8_ini_is_replaced_by_one_line`: write `# … \xe8 …\n[display]\noutput_res = 2560x1440\n[audio]\nvolume =
   150\n`, `Ini::load` → `set("motion","0","input")` → `save`; the file then reads `[input]\nmotion = 0` only.
2. `pad_value_with_newline_injects_a_key`: `settings::validate("player1_device", "pad:00ff\nfps_limit = 120:1")` is
   `Ok`; after `set` + `save`, `Ini::values()` holds `fps_limit = 120:1`.

## Second audit (2026-11-19)

An independent read-only review by codex-cli (gpt-6.2-astra) of revision e612ffb. Each finding below was checked in
the code before it was fixed; severity is ours. Findings A-28 to A-35 were checked on 2026-11-22.

| ID | Finding | Severity | Status |
|---|---|---|---|
| A-20 | The tag release build never received `AOD_DIAGNOSTICS_URL`, so CI binaries could not report even after the endpoint exists | Medium | Fixed — repository variable `AOD_DIAGNOSTICS_URL`, unset or empty keeps diagnostics off |
| A-21 | The Windows Bluetooth bridge DLL was searched up to four folders above each SDL location, outside the installation | Medium | Fixed — only `<engine>/nintendo`, the folder the game launch already uses |
| A-22 | The packager copied the whole staged engine and only checked names and extensions | Medium | Fixed — the copy must match `bundle.json` exactly (bytecode caches dropped), the ZIP is read back against the folder |
| A-23 | The pre-commit guard listed staged names but read working-tree bytes | Medium | Fixed — sizes and contents come from the index blobs |
| A-24 | Moving early RecompLauncher settings could follow a link out of the chosen game folder before deleting the file | Medium | Fixed — links are never followed and count as other files |
| A-25 | `engine.py fetch` accepted any branch `aod` on the pin, even a partial or stale patch queue | Medium | Fixed — commits are compared with `engine/patches` by patch id in `fetch` and `stage` |
| A-26 | A re-run of the release workflow could replace assets of a published release | Medium | Fixed — uploads go only into a draft, and the tag must equal `v` + `VERSION` |
| A-27 | NUP metadata was limited per package only | Low | Fixed — at most 256 packages and 64 MiB of metadata in total |
| A-33 | The menu preview test named one maintainer's toolchain folder | Low | Fixed — engine patch 0021: `AOD_TOOLCHAIN_DIR` or the toolchain beside the checkout |
| A-28 | Turning diagnostics off may race an upload already authorised | Low | Checked — the choice is read again right before each upload, so only an upload already being written can still arrive; left as it is |
| A-29 | `hwaoi.ini` and config writes use fixed temporary names without a cross-process lock | Low | Fixed — each process writes a temporary file of its own name, so none renames another's; two writers at once still mean the last one wins |
| A-30 | Play forced `HWAOI_PIPE_PREBUILD=1`, so "Prepare shaders early" could not be turned off | Medium | Fixed — no override; the runtime default stays on and `hwaoi.ini` decides |
| A-31 | A colour profile chosen in the launcher or RecompLauncher wrote only `color_preset`, which the picture ignored | Medium | Fixed — engine patch 0022: a named profile sets the four values, custom uses the player's |
| A-32 | Patch 0016 counts a skipped depth/stencil copy as handled | Low | Checked, left as it is: see the third audit |
| A-34 | Setup: repeated NUP scan without results, Vulkan 1.3 shown as passed without a GPU, cancel lost during preflight | Medium | Fixed — one search per visit, the check asks each card for its own Vulkan version, a Cancel before the engine has started is kept |
| A-35 | Smaller items: NUP index checks, optimistic UI after a failed save, ninja licence, English backend text in the Italian UI | Low | Fixed, but the last: folder indices are checked, a failed save shows the file's value again, the bundle carries Ninja's licence. Backend messages are still English in the Italian UI |

## Third audit (2026-11-22, before 0.2.0; prepared as 0.1.3)

A read of the launcher's backend by hand and a second independent read-only review by codex-cli (gpt-6.2-astra) of
the tracked files. Each finding was checked in the code before it was changed; severity is ours.

| ID | Finding | Severity | Status |
|---|---|---|---|
| A-36 | Copying a save into a new game folder: a finished save that waited in `save-tx` was copied under its final name, so a copy cut short could be loaded as the newest save, and the error was dropped | High | Fixed — everything is copied into a folder of the attempt's own and renamed into place when it is whole; a failure is reported and tried again at the next start |
| A-37 | The same copy deleted `save/<account>.partial` without knowing who made it | Medium | Fixed — only the attempt's own folder is removed, and one left by a crash only after an hour |
| A-38 | A folder holding only `save-tx/new` or `old` counted as having no save, and a first save that still waited was not taken | Medium | Fixed — the runtime's own rule for what a commit left |
| A-39 | A DLSS file the player picks was read, and then the picked path was verified again: the bytes verified could differ from the bytes installed | Medium | Fixed — the launcher's own copy is what Windows verifies |
| A-40 | Settings written before the first build lost every key the launcher has no row for | Low | Fixed — such keys are carried over as they are |
| A-41 | `tools/release.py` could package an engine bundle staged from uncommitted files, or older than the repository's engine, once the tree was clean | Medium | Fixed — both are refused without `--allow-dirty` |
| A-42 | `aod-nup`: a folder listing could run into the name pool and read names as node ids; `children` and `dir_name` took any index | Medium | Fixed |
| A-43 | Logs were read whole and as UTF-8 for their last lines, the frame rate and the end shown after a failure | Medium | Fixed — the end is read as bytes |
| A-44 | The RecompLauncher check passed below level 3 as long as there was no error, and looked for the wrong folder on Linux | Medium | Fixed |
| A-45 | Opt-in diagnostics looked for Vulkan error names the game never writes: a lost graphics card and a stop for lack of memory were both "unexpected exit" | Medium | Fixed |
| A-46 | The engine bundle shipped libvpx, Vulkan Memory Allocator and Ninja without their licence texts: the toolchain step skipped a licence file it did not find | Medium | Fixed — the texts are there and a missing one stops `toolchain` and `stage` |
| A-47 | Free disk space was looked up by the longest mount point that is a prefix of the path, so `/mnt/games` matched a disk at `/mnt/game` | Low | Fixed — whole folder names |
| A-48 | `aod-cli prepare` said nothing when the built game could not be recorded for the launcher's window | Low | Fixed — it says so; the build still counts |
| A-49 | `tools/menu_texts.py` looked for the menu's texts in a folder that is gone since the engine moved | Low | Fixed |
| A-50 | HDR output has rows in the game's menu only, and the language values `zh-CN` / `zh-TW` the runtime accepts are unknown to the launcher | Low | Fixed — four HDR rows in the catalogue (Picture), so in the launcher and in RecompLauncher; `zh-CN` and `zh-TW` are shown and written as `zh-Hans` and `zh-Hant`. The game started on an HDR10 swapchain on an HDR screen here (600 frames, a clean exit); how the picture looks on it was not judged |
| A-32 | (second audit, checked now) a skipped depth/stencil copy counted as handled | Low | Checked, left as it is — `vk_exec_xfer.c copy_depth_stencil` skips on purpose a copy whose two sides may share memory (Vulkan forbids it), with a warning in the log the first time; the generic copy it would otherwise fall to cannot copy a combined depth/stencil image. Doing it right needs a copy through a temporary image: not done |
