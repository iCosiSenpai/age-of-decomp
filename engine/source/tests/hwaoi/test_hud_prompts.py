"""The game's own button pictures drawn again for the device in use (runtime/hud_prompts.c).

The real file is compiled with a fixture that stands in for the input, the settings and the font, and draws into an
empty atlas: no game and no game data. The fixture writes the atlas's alpha as a PGM; the boxes below are where the
game keeps each picture (the same numbers as the tables in hud_prompts.c).
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FONT = ROOT / "runtime/ui/fonts/Roboto-Medium.ttf"
SIDE = 1024
FACE = {"X": (5, 515, 42, 42), "Y": (59, 515, 42, 42), "A": (923, 479, 42, 42), "B": (977, 479, 42, 42)}
SHOULDER = {"L": (493, 574, 40, 32), "R": (545, 574, 40, 32), "ZL": (601, 572, 38, 36), "ZR": (655, 572, 38, 36)}
MENU = {"PLUS": (9, 627, 34, 34), "MINUS": (63, 627, 34, 34), "PLUS_DISC": (118, 628, 33, 33),
        "MINUS_DISC": (172, 628, 33, 33)}
CLICK = {"LCLICK": (59, 567, 41, 46), "RCLICK": (113, 567, 41, 46)}
DPAD = {"DOWN": (697, 619, 34, 34), "LEFT": (735, 619, 34, 34), "RIGHT": (773, 619, 34, 34), "UP": (811, 619, 34, 34),
        "DOWN2": (761, 515, 42, 44), "LEFT2": (814, 515, 43, 42), "RIGHT2": (869, 515, 44, 42), "UP2": (923, 531, 42, 44)}
DIAMOND = {"YA": (220, 515, 44, 42), "XB": (275, 514, 42, 44), "B": (329, 515, 42, 42), "Y": (383, 515, 42, 42),
           "A": (437, 515, 42, 42), "X": (491, 515, 42, 42)}
STICK = {"LSTICK": (977, 536, 41, 37), "RSTICK": (5, 572, 41, 37)}   # "move" and "look around"
EVERY = {**{"face " + k: v for k, v in FACE.items()}, **SHOULDER, **MENU, **CLICK, **DPAD, **STICK,
         **{"dots " + k: v for k, v in DIAMOND.items()}}


def compiler():
    named = os.environ.get("AOD_TEST_CC")
    if named:
        return named
    own = ROOT.parent.parent / ".cache/toolchain/windows/llvm/bin/clang.exe"   # the project's toolchain (Windows)
    return str(own) if os.name == "nt" and own.exists() else shutil.which("clang")


def blocks_of(box):
    """The box grown to whole 4x4 blocks: what a redrawn picture may touch."""
    x, y, w, h = box
    return x & ~3, y & ~3, (x + w + 3) & ~3, (y + h + 3) & ~3


class Atlas:
    def __init__(self, drawn, alpha):
        self.drawn, self.alpha = drawn, alpha

    def ink(self, box):
        """Texels of the box that are more white than not."""
        x, y, w, h = box
        return sum(1 for row in range(y, y + h) for a in self.alpha[row * SIDE + x:row * SIDE + x + w] if a > 127)

    def part(self, box):
        x, y, w, h = box
        return b"".join(self.alpha[row * SIDE + x:row * SIDE + x + w] for row in range(y, y + h))

    def outside(self, boxes):
        """Texels with any white outside the blocks of these boxes."""
        keep = bytearray(self.alpha)
        for box in boxes:
            x0, y0, x1, y1 = blocks_of(box)
            for row in range(y0, y1):
                keep[row * SIDE + x0:row * SIDE + x1] = bytes(x1 - x0)
        return sum(1 for a in keep if a)


class HudPrompts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix="aod-hud-prompts-")
        cls.exe = Path(cls.tmp.name) / ("fixture.exe" if os.name == "nt" else "fixture")
        cc = compiler()
        if not cc:
            raise RuntimeError("Set AOD_TEST_CC to clang")
        subprocess.run([cc, "-O1", "-Wall", "-Wextra", "-Werror", "-I", str(ROOT / "runtime"),
                        "-I", str(ROOT / "runtime/third_party/imgui"), str(ROOT / "runtime/hud_prompts.c"),
                        str(ROOT / "runtime/sha256.c"), str(ROOT / "tests/hwaoi/hud_prompts_fixture.c"),
                        "-o", str(cls.exe)] + ([] if os.name == "nt" else ["-lm"]), check=True)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def draw(self, mode, *state, font=FONT):
        pgm = Path(self.tmp.name) / "alpha.pgm"
        out = subprocess.run([str(self.exe), str(font), mode, str(pgm), *state], check=True, capture_output=True,
                             text=True).stdout.split()
        self.assertEqual(out[:2], ["patch", "0"])   # an empty texture is not the game's atlas
        data = pgm.read_bytes()
        self.assertTrue(data.startswith(b"P5\n1024 1024\n255\n"))
        return Atlas(int(out[3]), data[len(b"P5\n1024 1024\n255\n"):])

    def assert_only(self, atlas, groups):
        """The pictures of these groups are there, the others are not, and nothing is outside them."""
        drawn = {name: box for group in groups for name, box in group.items()}
        for name, box in EVERY.items():
            wanted = any(box == b for b in drawn.values())
            with self.subTest(picture=name):
                self.assertEqual(atlas.ink(box) > 40, wanted)
        self.assertEqual(atlas.outside(drawn.values()), 0)
        self.assertEqual(atlas.drawn, len(drawn))

    def test_the_games_own_pictures_stay_when_asked(self):
        self.assert_only(self.draw("game", "family=playstation", "layout=xbox", "labels"), [])

    def test_a_switch_controller_with_the_games_buttons_changes_nothing(self):
        self.assert_only(self.draw("auto", "family=nintendo", "layout=nintendo", "labels"), [])

    def test_an_xbox_pad_keeps_its_letters(self):
        # south is A on the pad and in the game: the letters are right. LB, RT, the two menu buttons and the dots that
        # show where a button is are not.
        atlas = self.draw("auto", "family=xbox", "layout=xbox", "labels")
        self.assert_only(atlas, [SHOULDER, MENU, {"dots " + k: v for k, v in DIAMOND.items()}])

    def test_a_pad_that_names_no_buttons_is_drawn_as_an_xbox_pad(self):
        named = self.draw("auto", "family=xbox", "layout=xbox", "labels")
        self.assertEqual(self.draw("auto", "family=generic", "layout=xbox").alpha, named.alpha)

    def test_a_playstation_pad_gets_its_four_signs(self):
        atlas = self.draw("auto", "family=playstation", "layout=xbox", "labels")
        self.assert_only(atlas, [{"face " + k: v for k, v in FACE.items()}, SHOULDER, MENU,
                                 {"dots " + k: v for k, v in DIAMOND.items()}])
        signs = [atlas.ink(box) for box in FACE.values()]
        self.assertEqual(len(set(signs)), 4)   # cross, circle, square and triangle cut different amounts out
        self.assertEqual(self.draw("playstation", "family=xbox", "layout=xbox", "labels").alpha, atlas.alpha)

    def test_a_maker_asked_for_without_a_gamepad_changes_nothing(self):
        self.assert_only(self.draw("playstation"), [])
        self.assert_only(self.draw("xbox", "keyboard"), [])

    def test_a_remapped_button_shows_the_button_it_is_on(self):
        plain = self.draw("auto", "family=playstation", "layout=xbox", "labels")
        moved = self.draw("auto", "family=playstation", "layout=xbox", "labels", "bind:Y=west", "bind:X=north")
        # the game's Y was the triangle and is the square now; X the other way round
        self.assertAlmostEqual(moved.ink(FACE["Y"]), plain.ink(FACE["X"]), delta=12)
        self.assertAlmostEqual(moved.ink(FACE["X"]), plain.ink(FACE["Y"]), delta=12)
        self.assertNotEqual(moved.part(FACE["Y"]), plain.part(FACE["Y"]))
        self.assertEqual(moved.part(FACE["A"]), plain.part(FACE["A"]))

    def test_one_remapped_button_redraws_its_whole_group(self):
        # a Switch controller with ZL on the A button: the four shoulder pictures are ours, the rest stays the game's
        atlas = self.draw("auto", "family=nintendo", "layout=nintendo", "labels", "bind:ZL=east")
        self.assert_only(atlas, [SHOULDER])
        # ZL is a disc with A now, ZR beside it a box with its name, in boxes of one size
        self.assertGreater(abs(atlas.ink(SHOULDER["ZL"]) - atlas.ink(SHOULDER["ZR"])), 20)

    def test_a_button_off_the_face_buttons_is_not_shown_as_a_dot(self):
        atlas = self.draw("auto", "family=nintendo", "layout=nintendo", "labels", "bind:A=lpaddle1")
        self.assert_only(atlas, [{"face " + k: v for k, v in FACE.items()}, {"dots A": DIAMOND["A"]}])

    def test_the_keyboard_gets_its_keys(self):
        atlas = self.draw("auto", "keyboard")
        singles = {"dots " + k: DIAMOND[k] for k in "BYAX"}   # two buttons at once have no key picture
        # the sticks are the four keys that move and the mouse
        self.assert_only(atlas, [{"face " + k: v for k, v in FACE.items()}, SHOULDER, MENU, CLICK, DPAD, STICK, singles])
        self.assertEqual(self.draw("auto", "keyboard", "family=xbox", "layout=xbox", "labels").alpha, atlas.alpha)

    def test_without_a_font_the_shapes_are_still_drawn(self):
        atlas = self.draw("auto", "family=xbox", "layout=xbox", "labels", font=Path(self.tmp.name) / "missing.ttf")
        self.assert_only(atlas, [SHOULDER, MENU, {"dots " + k: v for k, v in DIAMOND.items()}])


if __name__ == "__main__":
    unittest.main()
