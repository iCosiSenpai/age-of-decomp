# Changelog

What changes for players in each version. The GitHub release notes are copied from here. Every fix that answers a
report names who reported it: thank you all for testing and writing in.

## 0.1.2 (not released yet)

Build the game again once after installing this version. Setup keeps your settings and saves.

### RecompLauncher
- Setup works from start to finish: choose your .nup, press Set Up, and Play when the build is done. Before, the setup
  window refused the copy with "That copy didn't work". Thanks to a player on RecompLauncher who reported it on
  2026-11-19.
- The game comes to the front when you press Play, so your controller works right away. Before, the game opened
  behind the launcher and ignored the controller until you clicked its window. Thanks to a player on RecompLauncher
  who reported it on 2026-11-20, and to kenroark on RecompLauncher, who had to assign the controller again once the
  game had loaded.
- Big Picture starts the game in fullscreen straight away.
- Play checks your .nup and tells the game where it is, so the game keeps reading the right file. A missing or
  different file stops Play with a clear message (code 65).
- A Play that fails leaves a log that RecompLauncher sends with your report, and codes 70 and 71 come with a short
  explanation. Thanks to the players on RecompLauncher whose reports of 2026-11-19 arrived without a log.

### Picture
- New: NVIDIA DLAA (Display → Anti-aliasing) smooths jagged, shimmering edges at Native 3D resolution on RTX cards. It
  needs NVIDIA's own DLSS file, which Age of Decomp cannot include: the launcher downloads it from NVIDIA once you
  accept NVIDIA's licence, or uses a copy you choose, and RecompLauncher offers it during setup. DLAA is not used in
  split screen. The setting matches the one in @ripps818's FSR work upstream (Alano6678/hwaoi-recomp#25).
- 3D resolution → Console (1080p) now draws the picture the game's own way: a 1600×900 scene that the game's upscaler
  brings to 1080p.
- A colour profile chosen in the launcher or in RecompLauncher now changes the picture.
- Frame rate offers 30 and 60 FPS. 90 and 120 sped up the whole game, so a saved 90 or 120 now plays at 60.
- The Field of view setting is gone: this game version has no field of view patch, so it did nothing.
- Split screen copes with more depth and stencil copies. Thanks to @Waloumi and everyone who reported the split-screen
  crash upstream (Alano6678/hwaoi-recomp#19).

### Sound
- Master volume now changes the game's sound, up to 300%. Thanks to @Scaw14001, who asked for higher volume upstream
  (Alano6678/hwaoi-recomp#17).
- New: Mute in the background, and Audio buffer for sound that crackles or drops out.
- The game plays without sound when Windows has no audio output, instead of closing.

### Controllers
- The launcher hands a Switch 2 Pro Controller's Bluetooth connection over to the game.
- Xbox, DualShock 4 and DualSense button images in the launcher and the in-game menu.

### In-game menu
- A Restart button for the settings that apply at start: the game closes and opens again by itself.

### Stability
- Intel graphics on Windows: the first start no longer closes two seconds in, while it prepares the shaders.
- A crash inside a graphics driver or another library now ends with code 71 and a log line that names it, instead of
  closing without a word.

### Launcher
- Leaving the Display page while NVIDIA's DLSS file downloads no longer hides the download: the progress, then the
  result, are there when you come back.
- The first D-pad press is no longer lost on a page whose first control is disabled.

### Still being worked on
Thank you for these reports too:
- Code 70 on AMD Radeon RX 9070 and RX 9070 XT cards, likely the AMD problem followed upstream
  (Alano6678/hwaoi-recomp#21): @hellknight0000 (#10) and players on RecompLauncher (2026-11-19 and 2026-11-20).
- A game that does not start on a Ryzen Z1 Extreme handheld: a player on RecompLauncher (2026-11-19). This version's
  log will help find out why.
- Player 2 in split screen, who never joins, and then code 70: kenroark on RecompLauncher.
- Menus and map that shrink to the middle of the screen above 1080p: a player on RecompLauncher (2026-11-19).
- Ultrawide screens (21:9 and 32:9): @ahmadsultan1990 (#8).
- The bonuses for Age of Calamity and Tears of the Kingdom save data: @Konia1234-Pega (#1).
- A short freeze in the first Sync skill scene: a player on RecompLauncher (2026-11-20), who also wrote that the game
  runs smoothly on an RTX 3080 Ti, as another player did for an RTX 5070.

## 0.1.1 (2026-11-19)
- The in-game menu, the game window and the system report show Age of Decomp's own name, and the game shows the same
  version as the launcher.
- Discrete AMD Radeon RDNA 3 cards (RX 7000) get the Vulkan recorder thread off by default, from @PrimeWalkerVN's pull
  request upstream (Alano6678/hwaoi-recomp#20). Thank you!
- Updates in System → Updates download without a GitHub account.
- RecompLauncher: Easy Setup, direct Play and 44 settings.
- Play from the command line works when the output folder is a relative path.

## 0.1.0 (2026-11-18)
- First Windows preview: build the game from your own .nup with a guided setup, a redesigned in-game settings menu in
  English and Italian, updates in System, and a RecompLauncher profile with 44 settings.
