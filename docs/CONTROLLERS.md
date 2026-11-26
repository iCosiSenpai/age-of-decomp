# Controllers

Age of Decomp reads controllers through **SDL 3.4** — in the game (engine `runtime/input*.c`) and in the launcher
(`apps/launcher/src-tauri/src/gamepad.rs`, SDL loaded from the engine bundle). No Steam Input, no virtual drivers:
SDL's own HIDAPI drivers talk to the pads.

| Controller | Connection | Buttons/sticks | Rumble | Motion | Notes |
|---|---|---|---|---|---|
| Xbox 360 / One / Series | USB, Bluetooth, wireless adapter | ✅ | ✅ (+ trigger rumble on One/Series) | — | XInput / GameInput |
| DualSense, DualSense Edge | USB, Bluetooth | ✅ | ✅ | ✅ gyro + accel | HIDAPI PS5 |
| DualShock 4 | USB, Bluetooth | ✅ | ✅ | ✅ | HIDAPI PS4 |
| Switch Pro Controller | USB, Bluetooth | ✅ | ✅ HD rumble | ✅ | HIDAPI Switch |
| Joy-Con L/R, pair | Bluetooth | ✅ (pairs combined) | ✅ | ✅ | `PLAYERn_TYPE = dual` makes the game see a Joy-Con pair |
| Switch 2 Pro Controller | USB, Windows Bluetooth | ✅ | ✅ | ✅ | USB uses SDL + libusb. On Windows, open the compiler or game and press Sync for Bluetooth. |
| Joy-Con 2 | USB | ✅ | ✅ | ✅ | Switch 2 HIDAPI over libusb. Bluetooth Joy-Con 2 pairing is outside the included transport. |
| NSO GameCube (Switch 2) | USB, Bluetooth | ✅ | ✅ | — | HIDAPI GameCube |

## Rules

- **SDL build**: `SDL3.dll` in the engine bundle is SDL 3.4.20 built from the official source with
  `SDL_HIDAPI_LIBUSB=ON` (libusb loaded at run time from `libusb-1.0.dll`, shipped beside the game and the launcher's
  SDL). The game turns on every HIDAPI driver (`runtime/input.c` `hidapi_hints`); `SDL_*` environment variables still
  override them.
- **Switch 2 Pro Controller over Bluetooth** uses the included Windows transport adapted from iCosiSenpai's
  NxToWin. Close other controller bridge apps and press **Sync** next to the PC. One Pro Controller 2 can connect
  this way, with standard controls, motion and rumble. It requires no separate app, driver or Steam Input.

- **Confirm / back in the launcher**: the button marked **A** confirms: the right one on a pad that has A there
  (Switch controllers), the bottom one on the others (Xbox A, PlayStation ✕, the GameCube pad's big A); the other of
  the two goes back. The launcher asks SDL what is printed on the buttons (`SDL_GetGamepadButtonLabel`), as the game
  does. The hint bar and the page hints show the buttons of the device used last: the pad that was pressed, or the
  keyboard after a key or a mouse button (`pad-active` event, `app.lastUsed`).
- **In game**: every button is where a Switch controller has it, on every maker's pad: the game's A is the right
  button (B on an Xbox pad, circle on a PlayStation one) and its B the bottom one. The game shows the face buttons
  by position in many places (its combo list, the dots, the skill wheels), so this is the layout where the button it
  draws is the one to press; the pictures show what the pad prints there. The menus confirm with the bottom button
  (below). `PAD_LAYOUT` and its Button layout row are gone in 0.2.0 (its *A at the bottom* choice made the drawn
  button another than the one to press); an older settings file's value is ignored. Up to 0.1.2 a pad that is not a
  Nintendo one had A at the bottom. Xbox, DualShock 4 and
  DualSense button images and each maker's button names (LB, L1, ZL) appear in the launcher and in Age of Decomp's
  own menu. That menu is moved with the game's own A and B, so it confirms with the button the game confirms with.
- **Confirm and back in the game's menus**: `CONFIRM_BUTTON`, *Confirm button in menus*. `auto` (default): on an
  Xbox, PlayStation or other pad the menus confirm with the bottom button (the game's B) and go back with the right
  one (its A), as most PC games do; a Switch controller and the keyboard and mouse keep the game's way. `bottom`: so
  on every pad. `right`: the game's way everywhere. It follows the device in use, at once. Battle keeps every button
  where a Switch controller has it: the game asks for confirm and back as actions of their own (9 and 10 of its button
  table), apart from the battle's, and only those two change places (engine: the generated patch set
  `hwaoi_confirm`, switched by `runtime/input.c` `input_confirm_bottom`). The menus' button guide follows: its
  confirm and back icons change places (the guide's battle entries, dodge and special attack, keep theirs), and the
  pictures then show the buttons that do it. Texts that name a battle button (tutorials, the controls list) keep the
  game's icons, which are the battle buttons. Age of Decomp's own menu confirms with the same button. A guide already
  on screen when the device changes keeps its icons until the screen is opened again.
- **Mapping tools** (DS4Windows, DSX, Steam Input, reWASD, JoyToKey): they can send keys or mouse motion while the pad
  is used, for a touchpad, a gyro mouse or a button mapped to a key. Those keys must not make the keyboard the device
  in use, or the menus' confirm and the pictures would flip back to the keyboard's at every press (GitHub #18). The
  game takes keys pressed within 250 ms of the same player's pad input (`KB_WITH_PAD_KEYS_NS`), and mouse motion
  alone once a pad was used, as the pad's: they still act, and the pad stays the device in use. The log says so once
  per kind (`input: keys together with a gamepad's input`). A key pressed on its own later switches to the keyboard
  as before, and so does a mouse button.
- **The game's own button pictures** (its hints, tutorials and HUD) follow the device in use: `BUTTON_PROMPTS`,
  *Buttons shown in the game*, engine `runtime/hud_prompts.c`. The game keeps them in three textures: its glyph set,
  a white mask it tints, for hints, tutorials and most of the HUD; the battle HUD's own sheet, with copies of the
  four face buttons, a stick click, R and ZR (the discs of the skill wheel's buttons that have a skill); and the
  skill wheel's sheet, where the four letters are part of the wheel's dark discs. When one of them goes to the
  graphics card, the pictures that are wrong for the player are drawn again in it, in the game's style and with our
  own drawings only (discs, boxes, strokes, letters of Roboto Medium): nothing is copied from the game or from a
  controller's maker, and the game's files and memory are not changed.
  - `auto` (default): the pad used last, each game button shown as the button that does it: the letter or sign
    printed there as SDL reports it (A B X Y, cross circle square triangle), LB / RT or L1 / R2 for the shoulders,
    the two menu buttons, and the dots that show where a face button is. With the keyboard and mouse in use: the
    key of each game button, the four keys that move for the left stick and the mouse for the right one.
  - `xbox`, `playstation`, `nintendo`: one maker's buttons for a pad that reports another (a PlayStation pad behind
    an Xbox wrapper). `game`: the game's own pictures.
  - A Switch controller with the game's own buttons changes nothing. Pictures are redrawn by group (the four face
    buttons, the four shoulder buttons, plus and minus), so one style is never mixed inside a group. A binding or a
    setting changed in the game's menu shows at once, with no restart.
  - Not followed: the drawings of a whole Switch controller the game has, the stick and d-pad pictures on a gamepad
    (they look the same on every maker's pad), and player 2's own pad: one texture serves both players and shows the
    device used last.
  - Each texture is known by its SHA-256, so only this game version's are touched (`sheets` in `hud_prompts.c`,
    with the place of every picture; `engine/source/tests/hwaoi/test_hud_prompts.py` draws them without the game).
- **Players**: `PLAYER1_DEVICE` / `PLAYER2_DEVICE` = `auto | keyboard | none | pad:<SDL GUID>:<k>`, k counting the
  connected pads of that GUID from 0 (`runtime/input.c` `pad_value`). Launchers up to 0.1.2 counted from 1: the game
  still finds the pad such a value meant when it is the last of its model. The launcher's *Controls* page shows each
  device with the player it is and assigns it with one button.
- **The Controls page** (launcher, `src/lib/pages/Pads.svelte`) and the game menu's Controls page list the
  keyboard and mouse and every connected controller, with what each has. What the launcher says about the game
  (who plays with what, where each game button is, the default keys, whose buttons the game's pictures show) comes
  from `src/lib/controls.ts`, a mirror of `runtime/input.c`, `runtime/input_map.c` and `runtime/hud_prompts.c`:
  change both sides together. The settings are shared by every
  controller; a controller only lists the ones it has the hardware for.
- **Not used by the game**: touchpads, controller lights, the Home and Share / Capture buttons. Back buttons
  (paddles) have no game button until the player gives them one in the game menu.
- **Rumble** is on in Age of Decomp (`HWAOI_RUMBLE=1`; the upstream launcher forces it off). Strength 0–300 %,
  curve, Xbox trigger rumble in *Controls*.
- **While the game runs** the compiler gives the game exclusive input. The game reuses the compiler's open Switch 2
  Bluetooth connection through private pipes. A game launched directly opens its own connection.
- **RecompLauncher**: `RECOMPLAUNCHER_PAD` supplies an Xbox or PlayStation default before SDL detects the real
  controller. Gamepad input follows window focus by default so the launcher overlay can use the pad.

## Test matrix (fill in with real hardware — task C-5)

| Pad | Windows USB | Windows BT | Linux USB | Linux BT | Tester |
|---|---|---|---|---|---|
| Switch 2 Pro | ✅ 2026-11-17 (maintainer): SDL Switch 2 driver, rumble and motion | ✅ 2026-11-19: detected by the compiler and game; live handoff awaits hardware verification | | | in game |
| Joy-Con 2 pair | | | | | |
| Switch Pro | | | | | |
| DualSense | | | | | |
| DualShock 4 | | | | | |
| Xbox Series | | | | | |
