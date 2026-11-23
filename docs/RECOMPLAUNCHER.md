# RecompLauncher integration

Age of Decomp publishes a Windows portable ZIP from its public GitHub repository. The preview is a prerelease,
so RecompLauncher's team must enable its beta before catalog installation is available. Linux binaries are not published yet.

## Install directly from the project

1. In RecompLauncher choose **Add a Recomp** and paste `https://github.com/iCosiSenpai/age-of-decomp`.
2. Once the team enables the preview/beta release, install its Windows x64 ZIP. Until then, use the local-folder method below.
3. Supply your own NUP containing the base game and update 1.0.3 (NCM 196608).
4. Complete Easy Setup, then press Play. The game continues reading resources from that NUP.

Alternatively extract the ZIP and import the folder through **Settings → Developers → Add a build**.
The `recomplauncher.json` embedded in the release belongs beside `AgeOfDecomp.exe` and `aod-cli.exe`.
The profile exposes the settings of `docs/SETTINGS.md`, the setup step, direct Play, save paths and log folders.

## Catalog submission

Adding a project by URL works separately from a listing in the Catalog. The catalog form is at
<https://recomplauncher.com/submit?platform=pc>. GitHub projects require sign-in with the owning GitHub account.
A developer account also requires manual review by RecompLauncher. Submission or an account application does
not itself mean that a listing has been approved.

Use the public repository URL, the Windows asset pattern `AgeOfDecomp-v*-Windows-x64.zip`, Nintendo Switch 2,
and the player's NUP requirements above. The embedded profile supplies Easy Setup metadata. Describe the project
as a compiler/launcher built on HWAoI Recomp, keep the existing upstream credits, and use launcher demo art only.

## Verify a release

Run `py -I tools/recomplauncher_check.py --release out/release/AgeOfDecomp-v<version>-Windows-x64`.
This compiles RecompLauncher's own Developer-tab checks in a scratch worktree. An optional `--nup` checks the
player's build ID through its NUP reader. A new folder normally reports that no game has been set up and no
saves exist yet. It must otherwise report level 3 and zero errors.

The expected commands are `AgeOfDecomp.exe cli prepare --game <nup> --out <install>/game` and
`AgeOfDecomp.exe --play --game-dir <install>/game --game=<nup>`: one program for the setup step and the game, as
RecompLauncher's guide asks ("Ship one program"). `aod-cli.exe` takes the same commands in a console. The profile asks for the player's NUP as the named
input `nup` (`{input:nup}`): named inputs are RecompLauncher's one way to ask for a file from 0.90 beta 68 on, and its
top-level `game` section is deprecated there (beta 65 and 66 refused a `game` section next to named inputs: "That
copy didn't work: Choose your copy of the game first"). The setup step checks the file itself first (modules and
resource inventory) and stops with exit 3 and the reason, which RecompLauncher shows as the setup's error. The game
reads the player's NUP while it plays, so `start` passes it too. `--game` may point to the copy's new place; it must be
the copy of the build (same header, TOC and size), and the game then reads it there. Exit 64 means the build is
absent/incomplete; exit 65 means the player's NUP is missing or is another file. Updates preserve player settings and
saves.

RecompLauncher 0.90 beta 68 changed the format. The profile stays in the form that beta 67 and beta 68 both read until
beta 68 is what players have:

- `prepare.timeout` and `prepare.version` stay. Beta 67 and older stop a step without `timeout` after five minutes
  and use `version` to ask for a new build. Beta 68 reads past both: its step has no time limit and runs again by
  itself after every new release. Its schema refuses the two keys, so the daily check in
  `.github/workflows/recomplauncher.yml` validates the file without them.
- The input has no `check` yet. Beta 68 can check the picked file by its build ID (`check.nup_build_id`), but beta 67
  and older drop the whole profile for an input field they do not know.

A run of the setup step on a finished build of the same engine reuses every stage and ends in about ten seconds, so
beta 68's run after every release costs little; after an engine change it builds again what changed. The number in the
done marker (`aod-ready-v<n>.json`, from `packaging/prepare-version.txt`) changes when a build of an older release
cannot be played, which sends a player through Setup again in any launcher.

The in-game menu's Restart (a setting read only at start was changed) ends the game with exit 75; `--play`, `aod-cli
play` and the launcher window start it again in the same process, so RecompLauncher sees one session.

## Where it writes, and what the PC needs

RecompLauncher (from 0.90 beta 66) moves, backs up and uninstalls a game as its folder, and gives the game and its
setup step `RECOMPLAUNCHER_DATA`: `{data}`, the `recompdata` folder inside the install folder. Age of Decomp writes only
there and under `{install}`: the build, `hwaoi.ini`, the saves and the game's logs stay in `{install}/game`, and the
launcher's own files (`launcher.json`, pending settings, its logs) go to `RECOMPLAUNCHER_DATA` when it is set
(`AOD_DATA_DIR` still wins; without either, `portable.txt` or `%LOCALAPPDATA%\AgeOfDecomp`). The setup step uses no
temp files it needs later, never asks anything, and neither it nor `--play` updates Age of Decomp: only the launcher
window offers updates, and RecompLauncher starts the game with `--play`.

The profile's `system` says what a Windows build needs (Windows 10, Vulkan 1.3); the game's page shows it and
RecompLauncher warns on an older Windows without stopping the game. A `linux` part (glibc of the build system,
Vulkan 1.3) is added with the first published Linux build.

On Linux RecompLauncher never runs an AppImage: it unpacks it and starts `AppRun` from a plain folder, with `APPDIR`
set and `APPIMAGE` unset. Our `AppRun` finds its files from its own path, so it works the same both ways.
`python3 tools/release.py linux --appimage` packs the same AppDir as `AgeOfDecomp-v<ver>-Linux-x64.AppImage` and
`AgeOfDecomp-v<ver>-Linux-x64.tar.xz` (installed when an AppImage will not unpack); publish both together.
