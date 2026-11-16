# Third-party notices

Age of Decomp is GPL-3.0-or-later. It is built on, and ships, the work of these projects (their full licence texts are
in `licenses/` of a release and in the engine bundle's `source/THIRD_PARTY_NOTICES.md`):

| Project | Use | Licence |
|---|---|---|
| [HWAoI Recomp](https://github.com/Alano6678/hwaoi-recomp) by Alano6678, LibertyPrime27, fgabe and contributors | The engine: static recompiler, Horizon/NVN2 runtime, Vulkan renderer, compiler workflow. Pinned in `engine/upstream.json`, changed only through `engine/patches/` | GPL-3.0-or-later (MIT parts in its LICENSE-MIT) |
| [DKB Recomp](https://github.com/Lucas-blip-png/dkb-pc-recomp) and the TotK Recomp lineage | Upstream of the engine | GPL-3.0 / MIT |
| [Ryujinx / Ryubing](https://git.ryujinx.app/ryubing/ryujinx) | Shader translator (vendored by the engine) | MIT |
| [Eglantine](https://github.com/Eglantine-Community/Eglantine) | Ampere shader frontend (vendored by the engine) | GPL-3.0 |
| [SDL 3](https://libsdl.org) | Input, window, audio (game and launcher controllers) | zlib |
| [llvm-mingw](https://github.com/mstorsjo/llvm-mingw) | C/C++ toolchain shipped to build the game on the player's PC | Apache-2.0 with LLVM exception, mingw-w64 licences |
| [Python](https://www.python.org) + numpy, lz4, zstandard, capstone | Generator runtime | PSF, BSD, BSD, BSD, BSD |
| [Ninja](https://ninja-build.org) | Build tool | Apache-2.0 |
| [Opus](https://opus-codec.org), [libvpx](https://chromium.googlesource.com/webm/libvpx), [Zstandard](https://github.com/facebook/zstd), [zlib](https://zlib.net) | Audio, movies, decompression | BSD / BSD / BSD / zlib |
| [Vulkan headers and loader](https://github.com/KhronosGroup), [Vulkan Memory Allocator](https://github.com/GPUOpen-LibrariesAndSDKs/VulkanMemoryAllocator) | Graphics | Apache-2.0 / MIT |
| [AMD FidelityFX FSR 1](https://github.com/GPUOpen-Effects/FidelityFX-FSR) | Upscaling (in the engine) | MIT |
| [Dear ImGui](https://github.com/ocornut/imgui) | In-game menu (in the engine) | MIT |
| [Tauri](https://tauri.app), [Svelte](https://svelte.dev), [ash](https://github.com/ash-rs/ash), [libloading](https://github.com/nagisa/rust_libloading), [sysinfo](https://github.com/GuillaumeGomez/sysinfo), [ruzstd](https://github.com/KillingSpark/zstd-rs), [serde](https://serde.rs) | Launcher | MIT / Apache-2.0 |
| [NUP format](https://github.com/a55uka/nup) by a55uka | Container spec read by `crates/aod-nup` (own implementation) | MIT |
| [Cinzel](https://github.com/NDISCOVER/Cinzel), [Inter](https://github.com/rsms/inter) | Launcher fonts | SIL OFL 1.1 |
| Launcher navigation, i18n pattern and engine-pin workflow | From the maintainer's own [Deluxe8](https://github.com/iCosiSenpai/Deluxe8) | GPL-3.0-or-later |
| Toolchain pins (`tools/engine.py`) | From the maintainer's own [totk-pc-recomp](https://github.com/iCosiSenpai/totk-pc-recomp) | MIT |

Nothing from Nintendo or Koei Tecmo is included. *Hyrule Warriors*, *The Legend of Zelda* and *Nintendo Switch 2* are
trademarks of Nintendo; they are named only to say which game this software works with.
