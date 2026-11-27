"""The Steam Deck's device profile gives only values the settings table accepts (runtime/cfg.c DECK).

The real cfg.c is compiled with a fixture that gives it the platform calls it makes. Before 0.3.6 the profile was the
one of the engine's other games: game resolution 1280x800, which Age of Decomp's display code refuses (code 70 at every
start on a Steam Deck, or with the Device profile set to Steam Deck), and 40 FPS, which its frame rate code refuses
under the experimental frame-rate profile. No game and no game data.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def compiler():
    named = os.environ.get("AOD_TEST_CC")
    if named:
        return named
    own = ROOT.parent.parent / ".cache/toolchain/windows/llvm/bin/clang.exe"   # the project's toolchain (Windows)
    return str(own) if os.name == "nt" and own.exists() else shutil.which("clang")


class DeviceProfile(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix="aod-device-profile-")
        cls.exe = Path(cls.tmp.name) / ("fixture.exe" if os.name == "nt" else "fixture")
        cc = compiler()
        if not cc:
            raise RuntimeError("Set AOD_TEST_CC to clang")
        # cfg.c includes host.h, whose simd_fp.h trips -Wbitwise-instead-of-logical with clang's -Wall
        subprocess.run([cc, "-O1", "-Wall", "-Wextra", "-Werror", "-Wno-bitwise-instead-of-logical",
                        "-I", str(ROOT / "runtime"), str(ROOT / "runtime/cfg.c"), str(ROOT / "tests/hwaoi/device_profile_fixture.c"),
                        "-o", str(cls.exe)], check=True)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def settings(self, ini, deck_env=False):
        with tempfile.TemporaryDirectory(prefix="aod-game-") as game:
            (Path(game) / "hwaoi.ini").write_text(ini, encoding="utf-8")
            env = {k: v for k, v in os.environ.items() if not k.upper().startswith(("HWAOI_", "TOTK_", "DKB_"))}
            env.pop("SteamDeck", None)
            if deck_env:
                env["SteamDeck"] = "1"
            out = subprocess.run([str(self.exe), str(Path(game) / "hwaoi.exe")], env=env, capture_output=True,
                                 text=True, check=True).stdout
        rows, answer = {}, {}
        for line in out.splitlines():
            words = line.split()
            if words[0] == "row":
                rows[words[1]] = (words[2], words[3] == "1")
            else:
                answer[words[0]] = words[1]
        return answer, rows

    def check_profile(self, answer, rows):
        self.assertEqual(answer["profile"], "deck")
        _, desktop = self.settings("[settings]\ndevice_profile = desktop\n")
        changed = {key: value for key, value in rows.items() if desktop.get(key) != value and key != "DEVICE_PROFILE"}
        self.assertTrue(changed, "the Steam Deck profile sets no row")
        for key, (value, ok) in changed.items():
            self.assertTrue(ok, f"the Steam Deck profile gives {key} = {value}, which its row does not accept")
        self.assertEqual(answer["preset"], "1", f"game resolution {rows['OUTPUT_RES'][0]} is no preset of the game")

    def test_a_steam_deck_gets_values_the_table_accepts(self):
        # The settings file a first build writes (compiler/backend.py), on a Steam Deck (Steam sets SteamDeck=1)
        answer, rows = self.settings("[settings]\nrender_scale=100\nfps_limit=60\n", deck_env=True)
        self.check_profile(answer, rows)
        self.assertEqual(rows["OUTPUT_RES"][0], "1280x720")
        self.assertEqual(rows["RENDER_SCALE"][0], "100", "the player's own line wins over the profile")

    def test_the_profile_chosen_on_any_pc(self):
        answer, rows = self.settings("[display]\ndevice_profile = deck\n")
        self.check_profile(answer, rows)
        self.assertEqual(rows["FPS_LIMIT"][0], "60")

    def test_a_desktop_keeps_the_tables_defaults(self):
        answer, rows = self.settings("[settings]\n")
        self.assertEqual(answer["profile"], "desktop")
        self.assertEqual(rows["OUTPUT_RES"][0], "auto")


if __name__ == "__main__":
    unittest.main()
