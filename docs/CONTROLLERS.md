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
  SDL). The game turns on every HIDAPI driver (engine patch 0004); `SDL_*` environment variables still override them.
- **Switch 2 Pro Controller over Bluetooth** uses the included Windows transport adapted from iCosiSenpai's
  NxToWin. Close other controller bridge apps and press **Sync** next to the PC. One Pro Controller 2 can connect
  this way, with standard controls, motion and rumble. It requires no separate app, driver or Steam Input.

- **Confirm / back in the launcher**: Nintendo pads confirm with **A (right)** and go back with **B (bottom)**, as on
  the console; all others confirm with the bottom button (A / ✕) and go back with the right one (B / ○). The hint bar
  shows the glyphs of the first connected pad's family.
- **In game**: `PAD_LAYOUT` decides the mapping — `auto` (Nintendo positions for Nintendo pads, Xbox positions for
  the rest), `nintendo`, `xbox`, `xbox_prompts` (the button labelled A confirms on any pad). Xbox, DualShock 4
  and DualSense images adapt in the compiler and Age of Decomp's in-game settings menu. The original game HUD
  still uses its Nintendo art.
- **Players**: `PLAYER1_DEVICE` / `PLAYER2_DEVICE` = `auto | keyboard | none | pad:<SDL GUID>:<n>` (the n-th connected
  pad with that GUID). The launcher's *Controllers* page assigns them with one button.
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
