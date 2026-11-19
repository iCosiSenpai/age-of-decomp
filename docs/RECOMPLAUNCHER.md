# RecompLauncher integration

Age of Decomp publishes a Windows portable ZIP from its public GitHub repository. The preview is a prerelease,
so RecompLauncher's team must enable its beta before catalog installation is available. Linux binaries are not published yet.

## Install directly from the project

1. In RecompLauncher choose **Add a Recomp** and paste `https://github.com/iCosiSenpai/age-of-decomp`.
2. Once the team enables the preview/beta release, install its Windows x64 ZIP. Until then, use the local-folder method below.
3. Supply your own NUP containing the base game and update 3.0.0 (NCM 196608).
4. Complete Easy Setup, then press Play. The game continues reading resources from that NUP.

Alternatively extract the ZIP and import the folder through **Settings → Developers → Add a build**.
The `recomplauncher.json` embedded in the release belongs beside `AgeOfDecomp.exe` and `aod-cli.exe`.
The profile exposes 44 settings, preparation, direct Play, save paths and log folders. Setup version 3 asks
existing installations to rebuild for the new engine runtime.

## Catalog submission

Adding a project by URL works separately from a listing in Discover. The catalog form is at
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

The expected commands are `aod-cli.exe cli prepare --game <nup> --out <install>/game` and
`AgeOfDecomp.exe --play --game-dir <install>/game`. Exit 64 means the build is absent/incomplete; exit 65 means
the player's NUP moved or changed. Updates preserve player settings and saves. Rebuild through Setup when the
profile's preparation version changes.
