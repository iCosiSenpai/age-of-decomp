# Changelog

What changes for players in each version. The GitHub release notes are copied from here. Every fix that answers a
report names who reported it: thank you all for testing and writing in.

## 0.1.3 (not released yet)

Build the game again once after installing this version. Setup keeps your settings and saves.

### RecompLauncher
- Checked with RecompLauncher 0.90 beta 68 as well as beta 67. From beta 68 on, RecompLauncher runs the setup again by
  itself after every Age of Decomp update: when the game needs no new build, that takes about ten seconds.

### Picture
- Above 1080p the game's menus, map and movies fill the picture instead of sitting small in the middle. Thanks to a
  player on RecompLauncher who reported it on 2026-11-19, and to @ripps818, whose work it is
  (Alano6678/hwaoi-recomp#22).
- Fullscreen shows the whole picture from the start. Before, the game could open in fullscreen with the picture only
  in the top left corner, or with thin black bands, until F11 was pressed twice. Thanks to kenroark on RecompLauncher,
  who reported it on 2026-11-21.
- New: Game resolution → Automatic, the default. The game takes your screen's resolution when your graphics card has
  the memory for it: 1440p on a dedicated card with 12 GB of video memory, 4K on one with 16 GB, 720p on a screen
  smaller than 1080p, and 1080p otherwise. Game resolution's description now says how much video memory each size
  wants: a size your card has too little memory for may stop the game while it loads (at 4K the game's texture pool
  alone takes about 7 GB). Thanks to the player on RecompLauncher who tried 1440p and 4K on a 12 GB card and wrote
  what happened (2026-11-22).
- New: Upscaling (Display) with NVIDIA DLSS next to AMD FSR. Upscaling quality goes from DLAA, which only cleans the
  edges at full resolution, to Quality, Balanced and Performance, which draw the 3D scene smaller and scale it up for
  more speed. DLSS needs an RTX card and NVIDIA's own DLSS file, as DLAA did in 0.1.2; a saved DLAA setting becomes
  DLSS with DLAA quality. Asked for upstream by @lustrousdragon (Alano6678/hwaoi-recomp#29).
- New: Frame generation (Display) on GeForce RTX 40 and 50 cards. NVIDIA DLSS draws extra frames between the game's
  own, from 2x up to 6x: on an RTX 5070 Ti at 1440p the game's 60 frames per second reach the screen as 237 at 4x and
  359 at 6x. A factor your card cannot do is brought down to the highest it can. The game itself still runs at 60,
  and your input is felt a little later. It works with or without Upscaling, adds frames only while the game's
  window is in front, rests in the menu, in movies and on loading screens, and shows its frames without waiting for
  the screen (Screen sync is Off while it is on). It needs NVIDIA's own frame generation file: the launcher downloads
  it from NVIDIA when you turn the setting on, and in RecompLauncher the setup does when you tick NVIDIA frame
  generation. The frame rate overlay shows the frames shown and the ones drawn. A player on RecompLauncher asked for
  more than 60 frames per second on 2026-11-21: this is how far it goes while the game's own speed is tied to 60.
  Thank you.
- New from upstream, thanks to @ripps818: AMD FSR 3.1 and FSR 4 upscaling (Alano6678/hwaoi-recomp#25), SMAA edge
  smoothing (#23) and HDR10 output (#24).
- New: HDR output is in the launcher and in RecompLauncher too (Picture), with its three brightness values. It was
  only in the game's own menu. It gives an HDR screen an HDR picture, so the desktop's HDR does not dim or wash out
  the game; HDR has to be on for the screen in Windows.
- DLSS quality says that a moving character can leave a faint trail in dark places: the game gives DLSS only the
  camera's movement, not each character's. Turn Upscaling off where it bothers you. Thanks to the player on
  RecompLauncher who reported the blur behind Zelda on 2026-11-21 and checked that it goes with DLAA off.
- Fullscreen's description says what it is: a borderless window that fills the screen, as it has been since 0.1.2.
  Thanks to the same player, who asked for borderless fullscreen.
- New: an OLED colour profile (Picture): deeper blacks, a little more contrast and saturation.
- The game's window has its own icon, the launcher's seal in other colours, in the title bar, the taskbar and Alt+Tab.

### Controllers
- New: the launcher's Controls page shows every device the game can be played with: the keyboard and mouse, and each
  connected controller with its battery, what it has (rumble, trigger rumble, motion sensors, back buttons) and which
  player it is. For the device you select it shows where each of the game's buttons is, with the names printed on that
  controller (LB and RT on Xbox, L1 and Options on PlayStation, ZL and + on Nintendo) or the keys, only the options
  that controller can use, and a live test with a rumble test.
- The launcher's button hints follow the device you used last: the controller you pressed, or the keyboard and mouse
  after a key or a click. Before, they always showed the first controller's buttons.
- The game menu's Controls page lists the connected devices the same way, shows only the options the connected
  controllers have, and its hints follow the device in use too.
- A controller chosen for a player in the launcher is the one the game uses. The launcher counted controllers of the
  same model from 1 and the game from 0, so with two of them the choice could land on the other one or on none.
- New: the buttons the game shows are the ones you press. The pictures in the game's hints, tutorials and HUD follow
  the controller you are using: A, B, X and Y where your controller has them, LB and RT on an Xbox controller, cross,
  circle, square and triangle with L1 and R2 on a PlayStation controller, and the keys and the mouse when you play
  with the keyboard. A button you move in the game menu's Controls page shows where it is now, at once. Buttons shown
  in the game (Controls) can also show one maker's buttons whatever the controller says it is, or keep the game's own
  Switch pictures. Where the game draws a whole controller, it stays a Switch controller. Thanks to a player on
  RecompLauncher, who asked for PlayStation buttons and reported on 2026-11-21 that a button moved to Square kept its
  old picture.
- Buttons shown in the game is in the game menu's Controls page as well, so it can be changed while you play. The
  Button layout row there no longer says that a button you move keeps its old letter.
- The Xbox prompts choice of Button layout is gone: in this game it only did what "A on the right" does. A saved Xbox
  prompts setting becomes "A on the right".
- The launcher confirms with the big A button of a GameCube controller.

### Launcher
- New: the Play page says when the game should be built again and why: after an engine update that needs it, when a
  rebuild is only on offer because the engine changed, when the build was stopped, or when your game file is no longer
  where it was. Each comes with its button.
- Display, Picture and Performance settings are named and explained in plainer words, and each Performance setting
  says what it really does.
- F5 starts the game from the keyboard, as the hint bar says.
- A settings file that names Chinese as zh-CN or zh-TW, as the game also accepts, shows in the launcher as 简体中文 or
  繁體中文 instead of an unknown value.
- Game resolution says that Automatic wants a dedicated graphics card, and Italian says Automatico wherever a
  description names that choice.
- The game's update is called 1.0.3 everywhere, the name the console shows. It was called 3.0.0, after the number
  the dump carries inside (196608). It is the same update.
- Setup's check of a .nup that holds the game as its 1.0.3 update alone, with no separate base game inside, says so
  under "Base game". It read "base game missing" next to a green tick. Such a file builds and plays like one with
  both parts.
- A setting that could not be saved no longer stays on screen as if it had been: the page shows the value the game
  will read, and Restore defaults says when it did not work.
- Setup looks for your .nup once when you open the step (with none on the PC it searched again and again), its
  Vulkan 1.3 check asks the graphics card itself instead of trusting that Vulkan is installed, and Cancel pressed in
  the first instant of a build stops it.
- A line you wrote by hand into the game's settings file before the first build is kept by the build.
- A DLSS or frame generation file you pick is checked as the copy the launcher keeps, not as the file it came from.
- Anonymous diagnostics, for players who turned them on, name a lost graphics card and a stop for lack of memory as
  what they are. Both were counted as an unexpected exit.

### In-game menu
- Italian, Russian and Simplified Chinese have every text of the menu. Simplified Chinese is new, from upstream
  (Alano6678/hwaoi-recomp).
- The descriptions of the buttons say what this game does with them where that was checked, and nothing where it was
  not. They used to describe another game.
- Screen sync and the player rows use the same words as the launcher.
- HDR interface brightness says what it sets: the white of this menu and of the performance overlay. The game's own
  HUD belongs to the picture. HDR output names Windows next to Linux for what it needs.

### Game
- New: Bonuses for other games' saves (Settings → Game). It tells the game you have Age of Calamity and Tears of the
  Kingdom save data, so it gives the bonus items it has for them as the story goes on. Off until you turn it on.
  Thanks to @Konia1234-Pega for asking (#1).
- A game folder that has no save yet takes the save of the folder this PC used before: the standalone launcher's game
  folder when you move to RecompLauncher, or the folder you built into until now. It is copied the first time you
  press Play there, and nothing is deleted or overwritten. The copy goes into place only when all of it is there: one
  that is cut short (a full disk) leaves the folder as it was and is tried again at the next start. Before, a new game
  folder started the game from its beginning while your save stayed behind in the old one. Thanks to a player on
  RecompLauncher whose report of a lost save on 2026-11-21 led to it.
- A save folder you copy into the game folder yourself is found too. When the folder has one save and the game's own
  account has none, the game uses that save: also when you copied `save` without `account.uid`, or started the game
  there once before copying. The log says so in one line.

### Stability
- AMD Radeon, code 70: on Radeon cards with AMD's own driver the game no longer hands its drawing commands to the
  graphics driver from a thread of their own. With that thread the card could stop answering ("device lost") and the
  game ended with code 70, at start or minutes into play; DKB Recomp, which this engine comes from, saw the same and
  ended it this way. A fault in how that thread passed its work on is closed too: it was found and measured by
  @tibor-stick in MKW Recomp (adriadam10/mkw-recomp#281). This is not tried on a Radeon here and may not end every
  loss: one tester's RX 9070 XT stopped at the same place with the thread off. Performance → Graphics driver calls on
  their own thread turns it back on for more speed in crowded scenes, or off on any other card. Thanks to
  @hellknight0000 and @maioro (#10) and to the players on RecompLauncher who sent their reports and logs between
  2026-11-19 and 2026-11-21.
- New: Diagnostics → Draw call markers, for a graphics card that stops answering. With it on, the log names the draw
  calls the card was working on, with their shaders: what is needed to find the cause on a card we do not have. Off
  until you turn it on; it costs a little speed.
- When the game stops showing new frames for 20 seconds, its log says where every thread is. The game is left as it
  is; the report is for the next bug report of a freeze.

### Overlay
- The frame rate counter (F10) sits just under the game's small "NORMAL" label in the top right corner instead of
  covering it.

### Downloads
- `SHA256SUMS.txt` has Unix line ends, so `sha256sum -c` on Linux checks the download.
- The engine folder carries the licence texts of libvpx, Vulkan Memory Allocator and Ninja, which were missing.

### For builders
- The engine is developed in this repository from now on, in `engine/source`. It was imported from HWAoI Recomp at
  `c6c96e6` with everything Age of Decomp had changed; `engine/ORIGIN.md` names its authors. Building from source no
  longer needs `engine.py fetch`.
- `tools/release.py` refuses an engine bundle staged from uncommitted files or older than the repository's engine,
  `tools/engine.py` stops when a licence text of the bundle is missing, and the RecompLauncher check asks for level 3.
- Libraries: ruzstd 0.9 and sha2 0.11 in the NUP reader, zip 9, sysinfo 0.40 and libloading 0.9 in the launcher,
  and the workflows' actions at their current releases (Dependabot's proposals).
- `tools/smoke_game.py` moves the sticks too (`--stick`), for a scripted check that walks.

### Still being worked on
Thank you for these reports too:
- Ultrawide screens (21:9 and 32:9). The game draws everything for a 16:9 screen, so the camera and each part of the
  picture have to be changed one by one: @ahmadsultan1990 (#8) and a player on RecompLauncher (2026-11-21).
- Code 70 on AMD Radeon cards where this version's change is not enough, like the place in the first battle where an
  RX 9070 XT stops: @hellknight0000 and @maioro (#10) and players on RecompLauncher. A report made with Diagnostics →
  Draw call markers turned on is what helps most.
- Code 70 when pressing Play on a Ryzen Z1 Extreme handheld: @Ostarion77 (#12) and a player on RecompLauncher
  (2026-11-19). This version's change for AMD's own driver applies to its Radeon graphics too; the log of a start with
  this version says whether that was the cause.
- A freeze on one frame, with the sound going on, when Zelda's shield-breaking skill hits a rock enemy whose guard is
  broken: a player on RecompLauncher (2026-11-21). This version's log says where the game is stuck: please send it
  after the next freeze.
- The map cursor that moves slower than normal with the game at 1080p: a player on RecompLauncher (2026-11-21).
- The trail behind a moving character with NVIDIA DLSS, and 4K on graphics cards with less than 16 GB of video memory
  (both under Picture).

## 0.1.2 (2026-11-20)

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
- With RecompLauncher 0.90 beta 66 or newer, everything Age of Decomp writes stays in the game's folder, so it moves,
  backs up and uninstalls with the game. The game's page also lists what your PC needs: Windows 10 or newer and a
  graphics card with Vulkan 1.3.

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
- Setup builds the game again after you move your .nup to another folder or drive. Before, it stopped with "This
  output belongs to a different source NUP". It still has to be the same copy.
- The launcher asks once whether to share anonymous diagnostics: counts of crash and build-failure kinds and frame
  rates rounded to 5, with no logs and nothing personal. It stays off until you say yes, and System → Anonymous
  diagnostics changes it.

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
