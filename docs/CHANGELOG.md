# Changelog

What changes for players in each version. The GitHub release notes are copied from here. Every fix that answers a
report names who reported it: thank you all for testing and writing in.

**Not tested here:** I have no Linux PC, no SteamOS device, no AMD Radeon graphics card and no PlayStation
controller, so I cannot see whether Age of Decomp really works with them. For those I count on your tests and your
logs to find and fix problems: please report what you see, with the newest `game-*.log`. Every release's notes say so.

## 0.3.6 (not released yet)

Build the game again once after installing this version (RecompLauncher does it by itself at the first start). Setup
keeps your settings and saves.

### Graphics
- DLSS and DLAA now keep the image they rendered when NVIDIA Streamline warns that the video memory budget was exceeded.
  Thanks to @satoshi-banjiu for reporting and testing this on an RTX 4060 Laptop GPU.
- At 3840x2160 the Gallery's Music list showed every title mixed with the others. The game draws those titles through
  one texture of its own, and at that size they no longer fit in it. They are now laid out there at the menus' own
  size and scaled with the rest of the screen. Thanks to @The-Gaymer for the report and the screenshots (#23).

### Diagnostics
- With *Draw call markers* on, the log of a graphics card that stops answering (code 70) also names the game's compute
  passes, not only its draws, so a crash in one of them is no longer put on the draw before it. When the card stopped
  right after a draw, the log no longer says that draw had finished: it says the crash is in that draw or in what came
  right after it. For the Radeon crashes reported by @hellknight0000 (#10) and @benkra66 (#16), and @copy1857's runs
  on a Radeon 8060S (#10).

### Requirements
- The README now says that the game wants a graphics card with 8 GB of video memory. On a 6 GB card the game's
  4.6 GB block of graphics data does not fit there and the game runs at about 10 fps. Thanks to @FredericVincent0 for
  the report and the log (#33).

## 0.3.5 (2026-11-26)

Build the game again once after installing this version (RecompLauncher does it by itself at the first start). Setup
keeps your settings and saves.

### Controllers
- With DS4Windows, DSX, Steam Input or another tool that turns a controller into keys or mouse motion, the menus kept
  going back to confirming with the right button and the button pictures to the keyboard's: the keys the tool sent
  together with the pad's own buttons made the game think the keyboard was in use. Keys or mouse motion that come
  within a quarter of a second of the pad's own input now leave the pad the device in use. Thanks to @The-Gaymer for
  the report and the logs (#18).
- Rumble and motion controls reach Joy-Cons and a handheld-mode controller too: the game asked for them with the
  handle of another controller style and got nothing. Thanks to @bdzdq for the report
  (Alano6678/hwaoi-recomp#33).
- A burst of input while the game's own menu is busy no longer loses a button's release, which could leave a button
  held in the menu.
- New setting *Camera speed* (Controls, in the launcher and in the game's menu): lower it to turn the camera more
  slowly with the right stick and the mouse, down to 60 % of the game's own speed. A camera faster than the game's
  needs a change in the game itself and is still to come. Thanks to @The-Gaymer for asking (#19).

### Launcher
- The launcher speaks Simplified Chinese too: *System → Language → 简体中文*, or *Automatic* on a Chinese desktop
  (outside Taiwan, Hong Kong and Macau, as the game's own menu decides). The text is drawn with the system's Chinese
  font. The game's own settings menu (Esc) already had Simplified Chinese. Thanks to @zerocreator0 for asking (#25).
- Setup makes the temporary folder the compiler writes into when the one the system names does not exist, instead of
  failing with "unable to make temporary file" (RecompLauncher's setup step can name a folder it never made:
  elripalda/RecompLauncher#551).
- Settings changed quickly one after the other are saved in the order you made them, and a change the launcher could
  not save no longer shows "Saved" on the Controllers page.
- Two settings saved at the same moment no longer overwrite each other's file, which could lose one of them.

### Diagnostics
- With *Draw call markers* on, the log of a graphics card that stops answering (code 70) also says what came between
  the last draws (a barrier inside the pass, a pass that ended and began again, a QUADS conversion) and which of its
  own targets a draw reads, so one log tells more about a crash. For the Radeon crash in a fight reported by
  @benkra66 (#16).
- New diagnostics setting *Draws that read their own picture*: *Restart pass* is a test for Radeon cards whose
  driver stops answering (code 70). If it keeps your game running, please tell us in a report (#16, #10).

## 0.3.0 (2026-11-24)

Build the game again once after installing this version (RecompLauncher does it by itself at the first start). Setup
keeps your settings and saves. Windows and Linux have the same version and the same game.

### Linux and Steam Deck
- Age of Decomp now runs natively on Linux: `AgeOfDecomp-v0.3.0-Linux-x64.AppImage`, and a `.tar.xz` of the same
  folder. Like the Windows version it builds the game on your PC from your own .nup, with nothing to install: the
  compiler and every library the game needs are inside. It runs on SteamOS 3 and on any distribution with glibc 2.34
  or newer (Ubuntu 22.04, Debian 12, Fedora 36 or newer), with a Vulkan 1.3 driver.
- On a Steam Deck or another handheld with SteamOS or Bazzite, set it up and play through RecompLauncher: its setup and
  Play need no window from us. The launcher's own window needs WebKitGTK 4.1 from your system and says so when it is
  missing.
- Looking for your .nup, the launcher also searches the drives Linux mounts in /run/media and /media, such as a Steam
  Deck's SD card.
- The Linux build follows the model of DKB Recomp's self-contained Linux release (@Lucas-blip-png): LLVM's own
  compiler and Valve's Steam Runtime 3 SDK, so the game it builds runs on SteamOS and on every distribution since.
  Thanks to everyone who asked for an AppImage in Alano6678/hwaoi-recomp#14, among them @omegaxis64-pixel and
  @alucardxs.

### Controllers
- In the launcher, the controller's left and right now move sliders (volumes, render scale and the others): they
  only moved with the keyboard's arrow keys.
- In the game's Controls menu, giving an action the key it already has in that place no longer drops its other key.

### Display
- On Linux, a window size you choose comes out at that size on a desktop scaled above 100 %: at 150 % a 1920x1080
  window came out 1280x720.

### Stability
- The game no longer ends with code 70, "RomFS cache missing or truncated", when its folder was moved or renamed after
  the build: Play now tells it where its resource cache is.
- When the launcher cannot read its own settings as it starts, it says so with a Try again button instead of loading
  for ever.
- A Play pressed right after the game closes no longer leaves the controllers with the launcher, and a Cancel pressed
  as Setup starts is no longer forgotten.
- Two game starts in the same second no longer write into the same log, and on Linux a game that a crash or the system
  ended shows its code instead of none.
- Setup removes only empty folders when it puts saves back after a build, and a save copied from an earlier game
  folder comes with its account or not at all.

## 0.2.1 (2026-11-24)

Build the game again once after installing this version (RecompLauncher does it by itself at the first start). Setup
keeps your settings and saves.

### Picture
- A Sync Strike's close-up of the two characters' hands shows them lit by the strike's warm light, as on the console,
  no longer dark grey or black. The close-up drew at the console's 1600x900 while the rest of the game draws at your
  resolution, so the game measured its exposure and toned only its top-left corner; it now draws at your resolution
  too. Every pair's Sync Strike had it.

### Controllers
- On Xbox, PlayStation and other controllers the game's menus confirm with the bottom button (A, cross) and go back
  with the right one (B, circle), and their button guide shows them so. Battle keeps every button where a Switch
  controller has it, so the bottom button still dodges. A Switch controller and the keyboard keep the game's way.
  Change it in Controls: Confirm button in menus.
- A mouse cursor on the map, which 0.2.0 listed as being worked on, is not coming: the game never turns on the mouse
  support its engine has, it does not read a Joy-Con 2 used as a mouse, and its map cursor moves with the left stick.
  With the keyboard and mouse, the mouse still turns the camera and its buttons are game buttons you can change in
  Controls.

### Performance
- On a PC with 16 GB of RAM the game no longer drops to 5-10 frames per second in battle when little of Windows'
  memory is free as it starts. The game copies its 4.6 GB block of graphics data into the graphics card's own memory
  only when Windows has room for it (on Windows that copy counts against the same memory limit as RAM); it now also
  counts the room Windows can still give by enlarging a page file it manages itself. Without the copy the card reads
  that data over PCIe on every draw. New: Performance → Memory for the game's graphics data, among the advanced
  settings: Video memory keeps the copy whatever Windows has free, System memory never makes it. Thanks to
  @seeep1337 (#15), who sent the log and found that Video memory brings the game back to 30 fps and more.

### Stability
- A battle no longer ends with code 70 ("GPU QUADS indirect validation failed") when the graphics card leaves one
  of the game's quad draws unconverted: that draw is skipped and the log says so. Thanks to @benkra66 (#16), whose
  Radeon RX 9070 XT stopped there while fighting Beta Elite Constructs.

## 0.2.0 (2026-11-23)

Build the game again once after installing this version. Setup keeps your settings and saves.

Playing on an AMD Radeon card? Turn on Diagnostics → Draw call markers before you play: it is among the advanced
settings, in the launcher and in RecompLauncher. If the game stops with code 70, send the report with its log: with
the markers on, the log says what the card was drawing when it stopped, which is what is needed to fix it on cards
we do not have.

### RecompLauncher
- Made for RecompLauncher 0.90 beta 68: RecompLauncher checks the .nup you pick and refuses one without update 1.0.3
  right away, and it runs the setup again by itself after every Age of Decomp update: when the game needs no new
  build, that takes about ten seconds. Update RecompLauncher when it offers beta 68.
- RecompLauncher's setup runs AgeOfDecomp.exe itself, the program you start, as RecompLauncher's guide asks: one
  program for the setup and the game. aod-cli.exe is still there for a console.
- Setup no longer stops with code 4, "Output folder is not empty", when the game's folder holds saves before its first
  build, as RecompLauncher's cloud saves or a restored backup can put them there. Setup puts saves and logs aside for
  the build and back when it is over; a file it does not know stops it with that file's name. An update in place was
  never affected: the built game stays in its folder and is built again there.

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
- NVIDIA DLSS, DLAA and frame generation no longer turn a running character see-through in dark places. The game
  tells DLSS only how the camera moves, not each character: a character the camera follows stays in place on the
  screen, while DLSS mixed the ground behind it into it, and in the dark, where the two look alike, the character
  faded away. The game marks its characters in the picture, and now each of them gets its own movement, and where the
  picture of the frame before belongs to something else (the ground a character has just left, a rock that now hides
  it) DLSS starts again from the new frame there. A faint trail can still show, only in the very few scenes where
  Zelda holds a torch before a fight. Thanks to the player on RecompLauncher who reported the blur behind Zelda on
  2026-11-21 and checked that it goes with DLAA off.
- Fullscreen's description says what it is: a borderless window that fills the screen, as it has been since 0.1.2.
  Thanks to the same player, who asked for borderless fullscreen.
- HDR output and its brightness values are greyed out in the launcher while no screen of the PC has HDR turned on:
  the game would draw its usual picture then. Turn HDR on for the screen in Windows and come back to the launcher.
  RecompLauncher's own settings tab cannot know about the screen and keeps them as they are.
- Screen sync is greyed out while Frame generation is on, because frame generation shows its frames without it. Fit to
  the window and its Sharpness are advanced options now: they only act when the window is larger than the game
  resolution, and the name of the first choice, AMD FSR 1, read as if FSR were on next to DLSS.
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
- Changed from 0.1.2: every button is where a Switch controller has it, on every controller. On an Xbox or
  PlayStation controller the game's A was the bottom button and its X the left one. Now A is the right button (B on
  Xbox, circle on PlayStation), B the bottom one, X the top one and Y the left one: the normal attack is the left
  button and the strong one the top button, as on the console, and you confirm with the right button. The game shows
  its buttons by position in the combo list, in the skill wheel and in the dots of its hints, so with the old layout
  the button it drew was not the one to press. The Button layout setting is gone, so the drawn button is always the
  one to press: to confirm with the bottom button, move A and B in the game menu's Controls page. A layout saved by
  an earlier version is ignored.
- New: the buttons the game shows are the ones you press. The pictures in the game's hints, tutorials and HUD, the
  skill wheel included, follow the controller you are using: the letters an Xbox controller prints where it has them
  (B to confirm, X for the normal attack) with LB and RT, cross, circle, square and triangle with L1 and R2 on a
  PlayStation controller, and the keys and the mouse when you play with the keyboard. A button you move in the game
  menu's Controls page shows where it is now, at once. Buttons shown in the game (Controls) can also show one maker's
  buttons whatever the controller says it is, or keep the game's own Switch pictures. Where the game draws a whole
  controller, it stays a Switch controller. The small wheels of three buttons, like the one next to Preparativi in the
  camp, follow too: every texture of the game's interface was searched for the four letters to find them. Thanks to a
  player on RecompLauncher, who asked for PlayStation buttons and reported on 2026-11-21 that a button moved to Square
  kept its old picture.
- Buttons shown in the game is in the game menu's Controls page as well, so it can be changed while you play. The
  Button layout row there no longer says that a button you move keeps its old letter.
- The launcher confirms with the big A button of a GameCube controller.
- Changed from 0.1.2: new keyboard and mouse keys, made for a PC. The left mouse button is the normal attack and the
  right one the strong attack, Space dodges, Left Shift (or Z) guards and the middle mouse button is the right stick's
  click (C too). E is still the special move, R the special actions, F the ZR button, Q the L button, 1 to 4 change
  the character and Tab opens the game's menu. Keys you moved yourself stay where you put them.
- With the keyboard, the character wheel shows the keys that change the character (1 to 4) where it showed the
  controller's arrows.
- Changing Rumble strength or Rumble curve makes the connected controllers rumble at once with the new values, in
  the launcher and in the game menu (Trigger rumble too in the game menu), so you feel what you chose.
- Motion controls stays in the game menu's Controls page, greyed with the reason, when no connected controller has a
  gyro the game can read: Switch, Switch 2 and PlayStation controllers have one; Xbox controllers, and controllers
  that work as one, send none, even with a gyro inside. The launcher's Controls page shows Motion controls only for a
  controller that has one.

### Launcher
- The setup builds the game faster, in the launcher and in RecompLauncher: on a 20-thread processor the compiling
  went from about 5 minutes to under 3. It now uses all of your processor's threads but one in eight (it stopped at
  eight before), at a lower priority than your other programs, so Windows or Linux stays as quick as before while it
  works: a test program ran just as fast during a build as without one. The game's one very large function, which
  did not run at all in a test of play, is compiled with a few of the slowest optimisation steps left out.
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
- HDR output and its three brightness rows are greyed when the game's screen shows no HDR (HDR off in the system's
  display settings, or a screen without it), with what to turn on; the brightness rows also while HDR output is off.
  The launcher greys them the same way.
- Window size is greyed in fullscreen and says the size in use, the screen's: it is the size of the window, which
  fullscreen does not use.

### Game
- New: Bonuses for other games' saves (Settings → Game). It tells the game you have Age of Calamity and Tears of the
  Kingdom save data, so it gives the bonus items it has for them as the story goes on. On unless you turn it off.
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
- The game no longer stops with code 70 at the start of the main mission Purging the Shadows, about 70% into the story
  ("required NVN2 stage 1 of program 1538 could not be lowered"). Its effect uses four kinds of graphics instructions
  the engine could not translate yet. All 101,235 shaders of the game went through the engine's translation to find
  every other one like it: 26 more translate now, the ones of that mission among them. Thanks to @seeep1337 (#13), who
  found the instruction in the shader, and to @DracoBogard, who reported the same stop upstream
  (Alano6678/hwaoi-recomp#27).
- With NVIDIA DLSS on, the game's log no longer fills with one Streamline error four times a frame ("Cannot have
  undefined format": 220,000 lines and 37 MB in half an hour of play). A Streamline message that repeats is written
  five times, then once at every power of ten with its count.
- AMD Radeon, code 70: on Radeon cards with AMD's own driver the game no longer hands its drawing commands to the
  graphics driver from a thread of their own. With that thread the card could stop answering ("device lost") and the
  game ended with code 70, at start or minutes into play; DKB Recomp, which this engine comes from, saw the same and
  ended it this way. A fault in how that thread passed its work on is closed too: it was found and measured by
  @tibor-stick in MKW Recomp (adriadam10/mkw-recomp#281). This is not tried on a Radeon here and may not end every
  loss: one tester's RX 9070 XT stopped at the same place with the thread off. Performance → Graphics driver calls on
  their own thread turns it back on for more speed in crowded scenes, or off on any other card. Thanks to
  @hellknight0000 and @maioro (#10) and to the players on RecompLauncher who sent their reports and logs between
  2026-11-19 and 2026-11-22.
- New: Diagnostics → Draw call markers, for a graphics card that stops answering. With it on, the log names the draw
  calls the card was working on, with their shaders: what is needed to find the cause on a card we do not have. Off
  until you turn it on; it costs a little speed.
- The first time an effect drawn like a camera view appears, such as the cut-in of a Sync Strike, the game waits for
  its new shaders at most a quarter of a second a frame, instead of stopping for seconds: the effect's remaining parts
  are drawn as soon as they are ready, a few frames later. Effects drawn once and kept (lighting tables, shadows) still
  wait until they are complete. Not yet tried with a Sync Strike here: the log says when it happens.
- Setup now prepares, before it ends, every shader your game has already met: it translates them for your graphics
  card and has its driver compile them. The first start after an update, a new build or a moved game folder then
  does not do that work while you play. A new copy of the game has met none yet: the game prepares each shader the
  first time it needs it, as before, and remembers it for the next setup.
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
- The enemies' dots missing from the zoomed minimap above 1080p, while the whole-map view shows them. This version
  scales two more of the game's 2D shapes (rectangles and lines) with the picture, which is what the dots looked
  like they were missing; it is not yet seen in a battle.
- Pictures of the game's tutorials that are screenshots with Switch buttons in them keep those buttons.
- Characters' hands and arms are too dark in the close-up of a Sync Strike, where the console shows them lit by the
  strike's light.
- Confirming with the bottom button in the game's menus (A on Xbox, cross on PlayStation) while battle keeps the
  Switch positions. The game has no such setting and reads the same buttons in both, so the place where its menus read
  them has to be found first. Until then, A and B can be swapped everywhere in the game menu's Controls page.
- The game's own mouse support (the Joy-Con 2 used as a mouse, a USB mouse), which could move the cursor on the map:
  the game asks for a mouse, and for now Age of Decomp answers that none is connected.
- Some of the game's shaders still do not translate (running all of them through the engine listed them): none is
  known to be met in play. If one is, the game stops with code 70 and its log names the shader.
- 4K on graphics cards with less than 16 GB of video memory (under Picture).

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
