import json
from pathlib import Path
from types import SimpleNamespace
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import engine
import release


def bundle_fixture(root: Path, **extra) -> None:
    """An engine folder whose versions match this release (bundle.json + source/VERSION)."""
    (root / "source").mkdir(parents=True, exist_ok=True)
    (root / "source/VERSION").write_text(release.version() + "\n", encoding="utf-8")
    info = {"version": release.version(), "aod_prepare_version": int(release.PREPARE_VERSION), **extra}
    (root / "bundle.json").write_text(json.dumps(info), encoding="utf-8")


class StageGuardTests(unittest.TestCase):
    def test_dirty_engine_is_refused_before_stage_is_removed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            stage = root / "stage"
            stage.mkdir()
            sentinel = stage / "keep"
            sentinel.write_text("keep")
            with patch.object(engine, "ENGINE", root), patch.object(engine, "STAGE", stage), \
                    patch.object(engine, "git", return_value=SimpleNamespace(stdout=" M runtime/a.c\n")):
                with self.assertRaises(SystemExit):
                    engine.cmd_stage(SimpleNamespace(allow_dirty=False))
            self.assertEqual(sentinel.read_text(), "keep")

    def test_engine_that_differs_from_the_queue_is_refused_before_stage_is_removed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            stage = root / "stage"
            stage.mkdir()
            (stage / "keep").write_text("keep")
            with patch.object(engine, "ENGINE", root), patch.object(engine, "STAGE", stage), \
                    patch.object(engine, "git", return_value=SimpleNamespace(stdout="")), \
                    patch.object(engine, "upstream", return_value={"commit": "base"}), \
                    patch.object(engine, "queue_mismatch", return_value="21 commit(s) on the pin, 20 patch file(s)"):
                with self.assertRaises(SystemExit):
                    engine.cmd_stage(SimpleNamespace(allow_dirty=False))
            self.assertEqual((stage / "keep").read_text(), "keep")

    def test_allowed_dirty_engine_is_recorded_in_bundle(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            tc, xlate, stage, own = root / "tc", root / "xlate", root / "stage", root / "own"
            (tc / "python/bin").mkdir(parents=True)
            (tc / "python/bin/python3").touch()
            xlate.mkdir()
            (xlate / "totk_xlate").touch()
            own.mkdir()
            def git(*args):
                return SimpleNamespace(stdout=" M runtime/a.c\n" if args[0] == "status" else "abc123\n")
            with patch.object(engine, "ENGINE", root), patch.object(engine, "TC", tc), \
                    patch.object(engine, "XLATE_OUT", xlate), patch.object(engine, "XLATE_EXE", "totk_xlate"), \
                    patch.object(engine, "STAGE", stage), patch.object(engine, "WIN", False), \
                    patch.object(engine, "OWN_SRC", own), patch.object(engine, "queue_mismatch", return_value=""), \
                    patch.object(engine, "git", side_effect=git), patch.object(engine, "source_files", return_value=[]), \
                    patch.object(engine, "copy_tree"), patch.object(engine, "run"), \
                    patch.object(engine, "upstream", return_value={"commit": "base"}):
                engine.cmd_stage(SimpleNamespace(allow_dirty=True))
            info = json.loads((stage / "bundle.json").read_text())
            self.assertTrue(info["dirty"])
            self.assertEqual(info["source_revision"], "abc123")
            self.assertEqual((stage / "source/VERSION").read_text(encoding="utf-8").strip(), release.version())


class ReleaseGuardTests(unittest.TestCase):
    def test_manifest_asks_for_the_copy_as_a_named_input(self):
        # RecompLauncher's Easy Setup (0.90 beta 65/66) refuses a `game` profile that also has named inputs when the
        # copy is picked in its window ("Choose your copy of the game first", player report RL-1), so the copy is the
        # first required input of our own, which RecompLauncher keeps as the game's copy, and both steps use it.
        m = release.manifest()
        self.assertNotIn("game", m)
        first = m["inputs"][0]
        self.assertEqual((first["id"], first.get("optional", False), first.get("shared", "")), ("nup", False, ""))
        self.assertEqual(first["formats"], [{"type": "file", "extensions": ["nup"]}])
        self.assertIn("{input:nup}", m["prepare"]["args"])
        self.assertIn("--game={input:nup}", m["start"]["args"])
        self.assertNotIn("{game}", json.dumps(m))

    def test_dirty_repository_requires_opt_in_and_is_recorded(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bundle_fixture(root, source_revision="engine-rev")
            def git(cmd, **kwargs):
                return SimpleNamespace(stdout=" M file\n" if "status" in cmd else "repo-rev\n")
            with patch.object(release.subprocess, "run", side_effect=git):
                with self.assertRaises(SystemExit):
                    release.release_info(False, root)
                info = release.release_info(True, root)
            self.assertEqual(info["repository_revision"], "repo-rev")
            self.assertEqual(info["engine_source_revision"], "engine-rev")
            self.assertTrue(info["dirty"])

    def test_clean_repository_is_recorded_as_clean(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bundle_fixture(root, source_revision="engine-rev", dirty=True)
            def git(cmd, **kwargs):
                return SimpleNamespace(stdout="" if "status" in cmd else "repo-rev\n")
            with patch.object(release.subprocess, "run", side_effect=git):
                info = release.release_info(False, root)
            self.assertFalse(info["dirty"])
            self.assertTrue(info["engine_dirty"])

    def test_engine_copy_must_match_its_bundle(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "sub").mkdir()
            (root / "a.txt").write_text("a")
            (root / "sub/b.txt").write_text("b")
            files = {rel: release.sha256(root / rel) for rel in ("a.txt", "sub/b.txt")}
            (root / "bundle.json").write_text(json.dumps({"files": files}), encoding="utf-8")
            (root / "sub/__pycache__").mkdir()
            (root / "sub/__pycache__/b.cpython-314.pyc").write_bytes(b"cache")
            self.assertEqual(release.verify_engine(root), 2)
            self.assertFalse((root / "sub/__pycache__").exists(), "bytecode caches are dropped")
            (root / "logs").mkdir()
            (root / "logs/game-1.log").write_text("a log written after staging")
            with self.assertRaises(SystemExit):
                release.verify_engine(root)
            (root / "logs/game-1.log").unlink()
            (root / "a.txt").write_text("changed")
            with self.assertRaises(SystemExit):
                release.verify_engine(root)
            (root / "a.txt").write_text("a")
            (root / "sub/b.txt").unlink()
            with self.assertRaises(SystemExit):
                release.verify_engine(root)

    def test_zip_must_hold_exactly_the_release_folder(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            folder = root / "AgeOfDecomp-v0-Windows-x64"
            (folder / "engine").mkdir(parents=True)
            (folder / "AgeOfDecomp.exe").write_bytes(b"exe")
            (folder / "engine/bundle.json").write_text("{}")
            z = root / "release.zip"
            with zipfile.ZipFile(z, "w") as zf:
                for p in sorted(folder.rglob("*")):
                    if p.is_file():
                        zf.write(p, Path(folder.name) / p.relative_to(folder))
            release.verify_zip(z, folder, folder.name)
            with zipfile.ZipFile(z, "a") as zf:
                zf.writestr(f"{folder.name}/extra.log", "not in the folder")
            with self.assertRaises(SystemExit):
                release.verify_zip(z, folder, folder.name)


if __name__ == "__main__":
    unittest.main()
