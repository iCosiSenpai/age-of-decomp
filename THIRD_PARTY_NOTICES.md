# Third-party notices

Age of Decomp is GPL-3.0-or-later. It is built on, and ships, the work of these projects (their full licence texts are
in `licenses/` of a release and in the engine bundle's `source/THIRD_PARTY_NOTICES.md`):

| Project | Use | Licence |
|---|---|---|
| [HWAoI Recomp](https://github.com/Alano6678/hwaoi-recomp) by Alano6678, LibertyPrime27, fgabe and contributors | The engine: static recompiler, Horizon/NVN2 runtime, Vulkan renderer, compiler workflow. Pinned in `engine/upstream.json`, changed only through `engine/patches/` | GPL-3.0-or-later (MIT parts in its LICENSE-MIT) |
| [HWAoI Recomp PR #20](https://github.com/Alano6678/hwaoi-recomp/pull/20), PrimeWalkerVN / PrimeWalker, commit `c95b6f6478c41f5ab186e4d48e0bdbb8f67affdd` | Discrete AMD RDNA 3 recorder default in `runtime/vk.h`, `vk_caps.c`, `vk_rec.c`; imported with original author in engine patch 0008 | GPL-3.0-or-later project, inherited MIT parts |
| [DKB Recomp](https://github.com/Lucas-blip-png/dkb-pc-recomp) (`42fcd9d`) | Base of the engine | GPL-3.0-or-later |
| [TotK Recomp](https://github.com/valentderah/totk-pc-recomp) by Valiantsin Dzerakh | Base of DKB Recomp: most of the engine, including the in-game menu's structure (`runtime/ui/`); also the pinned toolchain downloads in `tools/engine.py` (from its `tools/make_release.py`) | MIT |
| [LAS Recomp](https://github.com/peiikai/LASRecomp) by Peiikai (`da35b83`) | In the engine: lifter, code generator, host runtime, Vulkan back end, the in-game menu's Dear ImGui Vulkan set-up | MIT |
| [SavageWindRecomp](https://github.com/Revivedx/SavageWindRecomp) by Revivedx and contributors (`575ad88`) | In the engine: the in-game menu's page switching and left / right stepping, the performance overlay (F10) | MIT |
| [MacNativeGames](https://github.com/ly05010419/MacNativeGames) and the projects it builds on (d2r-mobile, nxnative, deko3d) | In the engine: host runtime, audio, RomFS, NVN front end, game patches (rows with commits in the engine's notices) | MIT / zlib |
| [Ryujinx](https://github.com/Ryujinx/Ryujinx) shader translator 1.3.2 | Shader translator (vendored by the engine, via MacNativeGames) | MIT |
| Eglantine Ampere lowering, from [Franseven/PokeDinner-a-Eglantine-Fork](https://git.eden-emu.dev/Franseven/PokeDinner-a-Eglantine-Fork) (`1dd40d8`), Copyright Eden Emulator Project | Ampere shader frontend (vendored by the engine) | GPL-3.0-or-later |
| [SDL 3](https://libsdl.org) | Input, window, audio (game and launcher controllers); built from source with HIDAPI libusb | zlib |
| [libusb](https://libusb.info) | Wired Switch 2 controllers through SDL (unmodified `libusb-1.0.dll`, MSYS2 package 1.0.30) | LGPL-2.1-or-later |
| NxToWin by iCosiSenpai (`752b214a047c21fec72253b03a3d308b39165929`) | Switch 2 Pro Controller Bluetooth protocol, motion and rumble code adapted in `apps/nintendo-bridge/Core/` | GPL-3.0-or-later in Age of Decomp |
| [.NET](https://github.com/dotnet/runtime) | Windows Bluetooth helper and shader translator run with the packaged .NET runtime. Runtime licence and notices are in `engine/licenses/`. | MIT, with separate notices for bundled components |
| [llvm-mingw](https://github.com/mstorsjo/llvm-mingw) | C/C++ toolchain shipped to build the game on the player's PC | Apache-2.0 with LLVM exception, mingw-w64 licences |
| [Python](https://www.python.org) + numpy, lz4, zstandard, capstone | Generator runtime (Linux: the relocatable [python-build-standalone](https://github.com/astral-sh/python-build-standalone) build; CPython licence in `engine/licenses/python-LICENSE.txt`, the libraries it links in, such as OpenSSL, SQLite and libffi, are listed by that project) | PSF, BSD, BSD, BSD, BSD |
| [Ninja](https://ninja-build.org) | Build tool | Apache-2.0 |
| [Opus](https://opus-codec.org), [libvpx](https://chromium.googlesource.com/webm/libvpx), [Zstandard](https://github.com/facebook/zstd), [zlib](https://zlib.net) | Audio, movies, decompression | BSD / BSD / BSD / zlib |
| [Vulkan headers and loader](https://github.com/KhronosGroup), [Vulkan Memory Allocator](https://github.com/GPUOpen-LibrariesAndSDKs/VulkanMemoryAllocator) | Graphics | Apache-2.0 / MIT |
| [AMD FidelityFX FSR 1](https://github.com/GPUOpen-Effects/FidelityFX-FSR) | Upscaling (in the engine) | MIT |
| [Dear ImGui](https://github.com/ocornut/imgui) 1.91.9, unmodified | In-game menu (in the engine) | MIT |
| [NVIDIA Streamline](https://github.com/NVIDIA-RTX/Streamline) SDK v2.14.16: headers, unmodified, and the signed production `sl.interposer.dll`, `sl.common.dll`, `sl.dlss.dll` | DLAA (headers in the engine, `runtime/third_party/streamline/`; the three DLLs in the Windows engine bundle `engine/streamline/`, licence `engine/licenses/streamline-LICENSE.txt`). NVIDIA's `nvngx_dlss.dll` (RTX SDK licence) is never shipped: the player's own copy is used | MIT |
| [Tauri](https://tauri.app), [Svelte](https://svelte.dev), [ash](https://github.com/ash-rs/ash), [libloading](https://github.com/nagisa/rust_libloading), [sysinfo](https://github.com/GuillaumeGomez/sysinfo), [ruzstd](https://github.com/KillingSpark/zstd-rs), [serde](https://serde.rs) | Launcher | MIT / Apache-2.0 |
| [NUP format](https://github.com/a55uka/nup) by a55uka | Container spec read by `crates/aod-nup` (own implementation) | MIT |
| [AppImage type 2 runtime](https://github.com/AppImage/type2-runtime) | Start-up code of the Linux AppImage | MIT |
| [Cinzel](https://github.com/NDISCOVER/Cinzel), [Inter](https://github.com/rsms/inter) (unmodified variable fonts from [google/fonts](https://github.com/google/fonts)) | Launcher fonts; embedded in the in-game menu too | SIL OFL 1.1 |
| [Roboto](https://fonts.google.com/specimen/Roboto) Medium | Font file in the engine's source (the in-game menu's font before the redesign) | Apache-2.0 |
| Launcher navigation, i18n pattern and engine-pin workflow | Adapted from the maintainer's own [Deluxe8](https://github.com/iCosiSenpai/Deluxe8) (`b6f2594`) | GPL-3.0-or-later |

How each reuse was checked (files, commits, method): `docs/CODE_PROVENANCE.md`.

Nothing from Nintendo or Koei Tecmo is included. *Hyrule Warriors*, *The Legend of Zelda* and *Nintendo Switch 2* are
trademarks of Nintendo; they are named only to say which game this software works with.
