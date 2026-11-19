<p align="center">
  <img src="docs/assets/icon.png" alt="Age of Decomp icon" width="160">
</p>

<h1 align="center">Age of Decomp</h1>

<p align="center">
  <a href="https://recompstore.com/dev/icosisenpai"><img alt="Verified developer on RecompStore" src="https://recompstore.com/assets/press/verified-dark.svg"></a>
</p>

<p align="center">
  <b>Hyrule Warriors: Age of Imprisonment</b>, recompiled into a native Windows and Linux game from <b>your own</b> copy.<br>
  Easy step-by-step setup · complete graphics settings · native Xbox, PlayStation, Switch and Switch 2 controllers ·
  <a href="https://github.com/elripalda/RecompLauncher">RecompLauncher</a> ready
</p>

<p align="center">
  <img alt="Windows and Linux" src="https://img.shields.io/badge/windows%20%7C%20linux-x86--64-efe6d2?style=for-the-badge&labelColor=0f1115">
  <a href="LICENSE"><img alt="GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-4fd18b?style=for-the-badge&labelColor=0f1115"></a>
  <img alt="Vulkan" src="https://img.shields.io/badge/Vulkan-1.3-3fc6ff?style=for-the-badge&labelColor=0f1115">
  <img alt="English and Italian" src="https://img.shields.io/badge/UI-English%20%7C%20Italiano-e7b75a?style=for-the-badge&labelColor=0f1115">
</p>

> [!IMPORTANT]
> **Bring your own game.** Age of Decomp contains nothing from Nintendo or Koei Tecmo and never downloads game files,
> keys or firmware. You need your own dump of the game as a **`.nup` file with the base game and update 3.0.0
> (NCM 196608)**. The game is built **on your PC**; the result is for your own use and must not be shared.

## What it is

The game's ARM64 code is translated to C ahead of time and compiled for your processor, with a runtime that provides
the Switch 2's system services and draws through Vulkan (NVN 2 → Vulkan, Ampere shaders → SPIR-V). The engine is
[HWAoI Recomp](https://github.com/Alano6678/hwaoi-recomp) (built on [DKB Recomp](https://github.com/Lucas-blip-png/dkb-pc-recomp)),
pinned and patched here; Age of Decomp adds the launcher, the setup, the settings, native controller support,
packaging for Windows and Linux, and RecompLauncher integration.

| | |
|---|---|
| 🧭 **Five-step setup** | Pick your `.nup` (it is also found automatically), see every check pass, choose a folder, press *Build*: one progress bar, a list of stages, time left. Stop any time; continuing reuses everything already done. About **7 minutes** on a Core Ultra 7 265K. |
| 🖥️ **Every setting** | Resolution (720p–4K), window, V-Sync mode, frame rate, 3D resolution, FSR upscaling and sharpness, aspect, field of view, colour (presets or brightness/contrast/saturation/gamma), performance overlay, graphics card, language, console mode, volume, players, controller type, button layout, dead zone, motion, rumble strength/curve/triggers, mouse, and the engine's performance and memory options. |
| 🎮 **Controllers, natively** | SDL 3 supports Xbox, DualSense / DualSense Edge, DualShock 4, Switch Pro, Joy-Con and GameCube controllers. Switch 2 Pro and Joy-Con 2 work over USB with our SDL build. On Windows, the Switch 2 Pro Controller can also connect over Bluetooth through the included transport. The compiler and its in-game settings menu show controller-specific button images. |
| 🚀 **RecompLauncher ready** | Each release carries `recomplauncher.json`: one-click setup from your `.nup` (recognised by its build ID), the settings on the game's page, Big Picture mode, save backups. |
| 🌐 **English and Italian** | Follows your system; switch any time. |

## Screenshots

<table>
  <tr>
    <td><img src="docs/assets/screenshots/home.png" alt="Play: status, quick settings, connected controllers"></td>
    <td><img src="docs/assets/screenshots/setup.png" alt="Setup wizard: six steps with a progress bar"></td>
  </tr>
  <tr>
    <td><img src="docs/assets/screenshots/display.png" alt="Display settings"></td>
    <td><img src="docs/assets/screenshots/controllers.png" alt="Controllers: live tester, rumble test, player assignment"></td>
  </tr>
</table>

<sub>The launcher with sample data (`?lang=en` demo mode): no game footage.</sub>

## Status

Early. The game builds, boots, plays its opening and reaches exploration and combat at 1080p / 60 FPS on tested PCs
(engine status: [HWAoI Recomp](https://github.com/Alano6678/hwaoi-recomp#current-status)). Known limits inherited from
the engine: 90/120 FPS also speed the game up; 1440p and 4K are experimental; a full playthrough is not verified yet. The v0.1.1 engine also includes the unmerged upstream RDNA 3 recorder
workaround from [PR #20](https://github.com/Alano6678/hwaoi-recomp/pull/20); synthetic policy tests passed, while
physical AMD validation here is pending.
Linux: the AppImage builds the game from your `.nup` and boots it to its first menu (tested under WSL with a software
renderer); speed on real Linux PCs and on the Steam Deck is not tested yet.

## Requirements

Windows 10/11 x64 or Linux x86-64, a CPU with **AVX2**, a **Vulkan 1.3** GPU with `VK_EXT_external_memory_host`
(recent NVIDIA, AMD, Intel drivers), 16 GB RAM (32 GB recommended), about **25 GB** free for the build.
On Linux also: kernel 6.7 or later (older ones work, slower), glibc 2.39+ and WebKitGTK 4.1 for the launcher window,
and to build the game clang, lld, Ninja and the SDL3, Opus, libvpx, zstd, Vulkan and zlib development packages.

## Quick start

1. Download `AgeOfDecomp-v<version>-Windows-x64.zip` from [Releases](../../releases) and unpack it anywhere.
2. Run **AgeOfDecomp.exe** and follow *Setup*. Keep your `.nup` where it is: the game reads its resources from it.
3. Press **Play**. In game: **Esc** settings, **F10** performance overlay, **F11** fullscreen.

The published preview currently ships Windows. Linux builds are experimental and can be built from source.
On Debian/Ubuntu install the build packages first:

```bash
sudo apt install clang lld ninja-build libsdl3-dev libopus-dev libvpx-dev libzstd-dev libvulkan-dev zlib1g-dev
```

With **RecompLauncher**: import the extracted folder through **Settings → Developers → Add a build**.
Drop in your `.nup`, follow Setup, then press Play. The repository can be added through **Add a recomp**;
catalog distribution of this prerelease needs the RecompLauncher team to enable its beta.
See [RecompLauncher integration](docs/RECOMPLAUNCHER.md).

## Updating

The compiler checks GitHub releases at startup. **System → Compiler updates** also has **Check for updates** and,
when a newer Windows package is available, **Update and restart**. The updater verifies the GitHub asset's SHA-256,
stages the package beside the installation, then replaces release-owned files after the compiler exits. It keeps
an installation backup in `.aod-update-*/backup`; the game folder, settings and saves are preserved. Close the game
and finish or cancel Setup before updating. The compiler asks you to rebuild only when an update changes the
game build requirements. Setup reuses your existing build folder, settings, and saves.

Diagnostics are optional and off until you opt in. See [what is collected](docs/DIAGNOSTICS.md).

This repository and its release downloads are public. Update checks and downloads work without a GitHub account.
An existing GitHub CLI login or `GH_TOKEN`/`GITHUB_TOKEN` can also be used. Credentials are never included in the
release or saved in launcher settings. Linux automatic installation is not available yet.

## Building from source

```powershell
py tools/engine.py toolchain     # pinned toolchain into .cache/ (llvm-mingw, SDL 3, Python, …)
py tools/engine.py fetch         # engine at engine/upstream.json + engine/patches
py tools/engine.py translator    # shader translator (.NET 10 SDK)
py tools/engine.py stage         # portable engine bundle → out/engine/windows
cd apps/launcher; pnpm install; pnpm tauri build --no-bundle
py tools/release.py windows --zip
```

Linux (the same steps with `python3`; `toolchain` checks the system packages and prints what is missing):

```bash
python3 tools/engine.py toolchain && python3 tools/engine.py fetch
python3 tools/engine.py translator && python3 tools/engine.py stage    # → out/engine/linux
(cd apps/launcher && pnpm install && pnpm build)
(cd apps/launcher/src-tauri && cargo build --release --locked --features tauri/custom-protocol)
python3 tools/release.py linux --appimage
```

Details: [AGENTS.md](AGENTS.md) (also the rules for AI agents working on this repository).

## Credits and licence

GPL-3.0-or-later. Built on HWAoI Recomp (Alano6678, LibertyPrime27, fgabe and contributors), DKB Recomp, Ryujinx,
Eglantine, SDL and more: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Age of Decomp is an independent, non-commercial fan project, not affiliated with or endorsed by Nintendo or
Koei Tecmo. *Hyrule Warriors*, *The Legend of Zelda* and *Nintendo Switch 2* are trademarks of Nintendo.
