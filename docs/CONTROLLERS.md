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
| Switch 2 Pro Controller, Joy-Con 2 | USB (Bluetooth: see notes) | ✅ | ✅ | ✅ | HIDAPI Switch 2 over libusb. Age of Decomp builds SDL 3.4.20 with libusb itself (`tools/engine.py toolchain`): the official Windows SDL3.dll has no Switch 2 driver |
| NSO GameCube (Switch 2) | USB, Bluetooth | ✅ | ✅ | — | HIDAPI GameCube |

## Rules

- **SDL build**: `SDL3.dll` in the engine bundle is SDL 3.4.20 built from the official source with
  `SDL_HIDAPI_LIBUSB=ON` (libusb loaded at run time from `libusb-1.0.dll`, shipped beside the game and the launcher's
  SDL). The game turns on every HIDAPI driver (engine patch 0004); `SDL_*` environment variables still override them.
- **Switch 2 pads over Bluetooth** (BLE) are not verified with SDL on Windows yet; wired goes through libusb. If they
  do not show up wirelessly, the maintainer's NxToWin bridges them (task C-5 records what works).

- **Confirm / back in the launcher**: Nintendo pads confirm with **A (right)** and go back with **B (bottom)**, as on
  the console; all others confirm with the bottom button (A / ✕) and go back with the right one (B / ○). The hint bar
  shows the glyphs of the first connected pad's family.
- **In game**: `PAD_LAYOUT` decides the mapping — `auto` (Nintendo positions for Nintendo pads, Xbox positions for
  the rest), `nintendo`, `xbox`, `xbox_prompts` (the button labelled A confirms on any pad).
- **Players**: `PLAYER1_DEVICE` / `PLAYER2_DEVICE` = `auto | keyboard | none | pad:<SDL GUID>:<n>` (the n-th connected
  pad with that GUID). The launcher's *Controllers* page assigns them with one button.
- **Rumble** is on in Age of Decomp (`HWAOI_RUMBLE=1`; the upstream launcher forces it off). Strength 0–300 %,
  curve, Xbox trigger rumble in *Controls*.
- **While the game runs** the launcher releases every controller (SDL shut down), so the game owns them; it takes
  them back when the game exits.
- **RecompLauncher**: `RECOMPLAUNCHER_PAD` is informational; `PAD_LAYOUT=auto` already follows the pad in use.

## Test matrix (fill in with real hardware — task C-5)

| Pad | Windows USB | Windows BT | Linux USB | Linux BT | Tester |
|---|---|---|---|---|---|
| Switch 2 Pro | ✅ 2026-11-17 (maintainer): detected by SDL's Switch 2 driver (named "Nintendo Switch Pro Controller"), rumble + gyro + accelerometer at 250 Hz, live motion, launcher and game | | | | in game |
| Joy-Con 2 pair | | | | | |
| Switch Pro | | | | | |
| DualSense | | | | | |
| DualShock 4 | | | | | |
| Xbox Series | | | | | |
