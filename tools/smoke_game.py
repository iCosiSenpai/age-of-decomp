"""Start a built game in a background window, wait for a number of presented frames, close it by its window (Windows).

  py -I tools/smoke_game.py NAME [--frames N] [--timeout S] [--game DIR] [--profile DIR] [--shot]
                                 [--press BUTTON[+BUTTON]@SECONDS ...] [--stick LX,LY[,RX,RY]@SECONDS[:MS] ...]
                                 [KEY=VALUE ...]

For a check of a developer build after an engine change. NAME names the run: its log is
<game>/logs/game-smoke-NAME.log and its own files go to work/smoke/NAME (gitignored: a screenshot is a picture of the
game). KEY=VALUE are runtime settings of this run only (HWAOI_KEY). --profile: the save folder to use; the default is
a new scratch one, so no save of a player is touched (give a COPY of a save folder to start from a save). --press
A@40 presses A 40 seconds after the start through the engine's expiring input feed (no OS key presses); --stick
0,32767@60:3000 holds the left stick up for three seconds from second 60 (axes -32767..32767, at most 10 s; a press
at the same moment replaces it: one command is held at a time); --shot keeps
the latest frame as a PNG. Prints the exit code and the log lines that matter. The game is never killed: it is closed
by its window, so its log is complete. The environment is the launcher's (apps/launcher/src-tauri/src/game.rs).
"""
import argparse
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUTTONS = {name: 1 << i for i, name in enumerate(("A", "B", "X", "Y", "LCLICK", "RCLICK", "L", "R", "ZL", "ZR", "PLUS",
                                                     "MINUS", "LEFT", "UP", "RIGHT", "DOWN"))}

parser = argparse.ArgumentParser()
parser.add_argument("name")
parser.add_argument("--frames", type=int, default=600)
parser.add_argument("--timeout", type=int, default=240)
parser.add_argument("--game", type=Path, default=ROOT / "work" / "game")
parser.add_argument("--profile", type=Path)
parser.add_argument("--press", action="append", default=[])
parser.add_argument("--stick", action="append", default=[])
parser.add_argument("--shot", action="store_true", help="keep the latest present as a PNG in the run folder")
parser.add_argument("settings", nargs="*")
a = parser.parse_args()

out = a.game.resolve()
binary = out / "work" / "build" / "hwaoi196608"
run = ROOT / "work" / "smoke" / a.name
run.mkdir(parents=True, exist_ok=True)
profile = (a.profile or run / "profile").resolve()
profile.mkdir(parents=True, exist_ok=True)
feed = run / "feed.txt"
if feed.exists():
    feed.unlink()
env = dict(os.environ)
for name in ("INPUT_SCRIPT", "INPUT_RECORD", "XLATE_ID", "PIPE_SYNC", "PIPE_SKIP", "PIPE_PREBUILD"):
    for prefix in ("HWAOI_", "DKB_", "TOTK_"):
        env.pop(prefix + name, None)
env.update({
    # as apps/launcher/src-tauri/src/game.rs
    "HWAOI_GPU": "vulkan", "HWAOI_INPUT": "auto", "HWAOI_RUMBLE": "1", "HWAOI_PROGRESS": "1",
    "HWAOI_EXEFS": str(out / "work" / "game" / "exefs"), "HWAOI_PROFILE_DIR": str(profile),
    "HWAOI_GSV_LOCAL_FRAME_POLICY": "1", "HWAOI_NATIVE_INFLATE": "1", "HWAOI_POOL_BACKPRESSURE": "1",
    "HWAOI_POOL_DIAG": "0", "HWAOI_IR_BANKS": "1",
    "HWAOI_XLATE": str(binary / "xlate-managed-irbanks" / "totk_xlate.exe"),
    "HWAOI_TRACE": "perf", "HWAOI_CORE_STALL_DIAG": "0",
    "DKB_SHADER_FAILURE_DUMP": str(out / "work" / "reports" / "shader-failures"),
    "HWAOI_PIPE_SYNC": "0", "HWAOI_PIPE_SKIP": "strict", "HWAOI_RATE_UNLOCK": "0",
    "HWAOI_ZLIB_DLL": str(binary / "zlib1.dll"),
    # this run only: a window behind the others, and the input feed
    "HWAOI_BACKGROUND_WINDOW": "1", "HWAOI_INPUT_FEED": str(feed),
})
if a.shot:
    (run / "shots").mkdir(exist_ok=True)
    env.update({"HWAOI_SHOTS": "w0-100000:2", "HWAOI_SHOTS_DIR": str(run / "shots"), "HWAOI_SHOTS_LATEST": "1"})
for kv in a.settings:
    key, value = kv.split("=", 1)
    env["HWAOI_" + key.upper()] = value
(out / "logs").mkdir(exist_ok=True)
log_path = out / "logs" / f"game-smoke-{a.name}.log"
log = open(log_path, "wb")
p = subprocess.Popen([str(binary / "hwaoi.exe"), "--play-experimental"], cwd=binary, env=env, stdin=subprocess.DEVNULL,
                     stdout=log, stderr=subprocess.STDOUT, creationflags=0x08000000 | 0x00000200)
# (seconds, button mask, four axes, milliseconds held)
presses = []
for s in a.press:
    names, at = s.split("@")
    mask = 0
    for b in names.split("+"):
        mask |= BUTTONS[b]
    presses.append((float(at), mask, (0, 0, 0, 0), 150))
for s in a.stick:
    axes, at = s.split("@")
    at, _, ms = at.partition(":")
    axes = [int(v) for v in axes.split(",")]
    presses.append((float(at), 0, tuple(axes + [0] * (4 - len(axes))), int(ms or 1000)))
presses.sort()
start = time.time()
seq = 0
frames = 0
while time.time() - start < a.timeout and p.poll() is None:
    time.sleep(1)
    while presses and time.time() - start >= presses[0][0]:
        seq += 1
        _, mask, axes, ms = presses.pop(0)
        tmp = feed.with_name(f"feed.{os.getpid()}.tmp")
        tmp.write_text(f"{seq} {mask:x} {axes[0]} {axes[1]} {axes[2]} {axes[3]} {ms} 0 0 0\n", encoding="ascii")
        os.replace(tmp, feed)
    text = log_path.read_bytes().decode("utf-8", "replace")
    found = re.findall(r"present=(\d+)", text)
    frames = int(found[-1]) if found else 0
    if frames >= a.frames and not presses:
        break
elapsed = time.time() - start
closed = "it ended by itself"
if p.poll() is None:
    # Close by the window, as a player does; the game then writes its exit lines.
    subprocess.run(["powershell", "-NoProfile", "-Command",
                    f"$p = Get-Process -Id {p.pid} -ErrorAction SilentlyContinue; if ($p) {{ [void]$p.CloseMainWindow() }}"],
                   capture_output=True)
    try:
        p.wait(60)
        closed = "closed by its window"
    except subprocess.TimeoutExpired:
        closed = "STILL RUNNING after the close request"
log.close()
text = log_path.read_bytes().decode("utf-8", "replace")
print(f"{a.name}: {frames} frames in {elapsed:.0f} s, {closed}, exit code {p.returncode}, log {log_path}")
keep = re.compile(r"vk_rec: (the|recorder|a part|no)|frametime: exit|FATAL|fatal:|device lost|device fault|resolution: |"
                  r"gfx: swapchain|vk: device \"|vk: GPU |prompts: |memory: |out of memory|vk: heap")
shown = 0
for line in text.splitlines():
    if keep.search(line) and shown < 40:
        print("  " + line[:230])
        shown += 1
recs = re.findall(r"vk_rec: (\d+) of (\d+) parts recorded by the recorder: (\d+) packets .*? (\d+) drains .*? waited ([\d.]+) ms",
                  text)
if recs:
    waits = [float(r[4]) / max(1, int(r[3])) * 1000 for r in recs]
    print(f"  recorder: {len(recs)} windows, mean wait per drain {sum(waits) / len(waits):.2f} us, "
          f"parts on the recorder {recs[-1][0]} of {recs[-1][1]}")
sys.exit(0 if p.returncode == 0 else 1)
