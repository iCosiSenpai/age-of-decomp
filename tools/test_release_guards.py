import json
from pathlib import Path
from types import SimpleNamespace
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import engine
import release


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

    def test_allowed_dirty_engine_is_recorded_in_bundle(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            tc, xlate, stage = root / "tc", root / "xlate", root / "stage"
            (tc / "python/bin").mkdir(parents=True)
            (tc / "python/bin/python3").touch()
            xlate.mkdir()
            (xlate / "totk_xlate").touch()
            def git(*args):
                return SimpleNamespace(stdout=" M runtime/a.c\n" if args[0] == "status" else "abc123\n")
            with patch.object(engine, "ENGINE", root), patch.object(engine, "TC", tc), \
                    patch.object(engine, "XLATE_OUT", xlate), patch.object(engine, "XLATE_EXE", "totk_xlate"), \
                    patch.object(engine, "STAGE", stage), patch.object(engine, "WIN", False), \
                    patch.object(engine, "git", side_effect=git), patch.object(engine, "source_files", return_value=[]), \
                    patch.object(engine, "copy_tree"), patch.object(engine, "run"), \
                    patch.object(engine, "upstream", return_value={"commit": "base"}):
                engine.cmd_stage(SimpleNamespace(allow_dirty=True))
            info = json.loads((stage / "bundle.json").read_text())
            self.assertTrue(info["dirty"])
            self.assertEqual(info["source_revision"], "abc123")


class ReleaseGuardTests(unittest.TestCase):
    def test_dirty_repository_requires_opt_in_and_is_recorded(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "bundle.json").write_text('{"source_revision": "engine-rev"}')
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
            (root / "bundle.json").write_text('{"source_revision": "engine-rev", "dirty": true}')
            def git(cmd, **kwargs):
                return SimpleNamespace(stdout="" if "status" in cmd else "repo-rev\n")
            with patch.object(release.subprocess, "run", side_effect=git):
                info = release.release_info(False, root)
            self.assertFalse(info["dirty"])
            self.assertTrue(info["engine_dirty"])


if __name__ == "__main__":
    unittest.main()
