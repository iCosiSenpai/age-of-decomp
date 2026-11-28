import json
from pathlib import Path
from types import SimpleNamespace
import sys
import tarfile
import tempfile
import unittest
import zipfile
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_content
import engine
import release

ROOT = Path(__file__).resolve().parents[1]


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

    def test_allowed_dirty_engine_is_recorded_in_bundle(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            tc, xlate, stage = root / "tc", root / "xlate", root / "stage"
            for tool in ("python/bin/python3", "llvm/bin/clang", "bin/ninja"):   # the Linux bundle's whole compiler
                (tc / tool).parent.mkdir(parents=True, exist_ok=True)
                (tc / tool).touch()
            xlate.mkdir()
            (xlate / "totk_xlate").touch()
            with patch.object(engine, "ENGINE", root), patch.object(engine, "TC", tc), \
                    patch.object(engine, "XLATE_OUT", xlate), patch.object(engine, "XLATE_EXE", "totk_xlate"), \
                    patch.object(engine, "STAGE", stage), patch.object(engine, "WIN", False), \
                    patch.object(engine, "git", return_value=SimpleNamespace(stdout=" M runtime/a.c\n")), \
                    patch.object(engine, "source_files", return_value=[]), patch.object(engine, "LINUX_LICENSES", ()), \
                    patch.object(engine, "copy_tree"), \
                    patch.object(engine, "run", return_value=SimpleNamespace(returncode=0)):
                engine.cmd_stage(SimpleNamespace(allow_dirty=True))
            info = json.loads((stage / "bundle.json").read_text())
            self.assertTrue(info["dirty"])
            self.assertEqual(info["source_revision"], engine.engine_revision(info["files"]))
            self.assertEqual((stage / "source/VERSION").read_text(encoding="utf-8").strip(), release.version())

    def test_engine_revision_follows_only_what_makes_the_game(self):
        base = {"source/runtime/a.c": "1", "source/compiler/backend.py": "2", "source/tools/x.py": "3",
                "source/VERSION": "v1", "source/tests/t.py": "4", "source/README.md": "5", "python/python.exe": "6"}
        rev = engine.engine_revision(base)
        self.assertEqual(len(rev), 40)
        # a release that changes only the version, a test, a notice or the bundled tools keeps the engine's id
        for path in ("source/VERSION", "source/tests/t.py", "source/README.md", "python/python.exe"):
            self.assertEqual(engine.engine_revision({**base, path: "changed"}), rev, path)
        self.assertEqual(engine.engine_revision({**base, "source/tests/new.py": "7"}), rev)
        # anything that goes into the game changes it
        for path in ("source/runtime/a.c", "source/compiler/backend.py", "source/tools/x.py"):
            self.assertNotEqual(engine.engine_revision({**base, path: "changed"}), rev, path)
        self.assertNotEqual(engine.engine_revision({**base, "source/recomp/new.py": "8"}), rev)


class ReleaseGuardTests(unittest.TestCase):
    def test_manifest_asks_for_the_copy_as_a_named_input(self):
        # Named inputs are RecompLauncher's one way to ask for a file (0.90 beta 68; its Easy Setup of beta 65/66
        # refused a `game` profile next to named inputs, player report RL-1), so the copy is the first required
        # input of our own, which RecompLauncher keeps as the game's copy, and both steps use it.
        m = release.manifest()
        self.assertNotIn("game", m)
        first = m["inputs"][0]
        self.assertEqual((first["id"], first.get("optional", False), first.get("shared", "")), ("nup", False, ""))
        self.assertEqual(first["formats"], [{"type": "file", "extensions": ["nup"]}])
        self.assertIn("{input:nup}", m["prepare"]["args"])
        self.assertIn("--game={input:nup}", m["start"]["args"])
        self.assertNotIn("{game}", json.dumps(m))

    def test_manifest_reads_in_the_recomplauncher_players_have(self):
        # RecompLauncher 0.90 beta 68, what players have since 2026-11-23: an input may carry `check`, and the setup
        # step has neither `timeout` nor `version` (its schema refuses both). The player's .nup is checked by its
        # build ID when it is picked.
        m = release.manifest()
        for row in m["inputs"]:
            self.assertLessEqual(set(row), {"id", "label", "role", "optional", "shared", "formats", "destination",
                                            "check"}, row["id"])
            for f in row.get("formats", []):
                self.assertLessEqual(set(f), {"type", "extensions", "requires"}, row["id"])
        self.assertNotIn("timeout", m["prepare"])
        self.assertNotIn("version", m["prepare"])
        self.assertEqual(m["prepare"]["done"], [f"{{install}}/game/aod-ready-v{release.PREPARE_VERSION}.json"])
        # the whole ModuleId of the update's main (`aod-nup build-id <dump>` prints it)
        self.assertRegex(release.BUILD_ID, r"^[0-9a-f]{40}$")
        self.assertEqual(m["inputs"][0]["check"], {"nup_build_id": release.BUILD_ID})

    def test_the_setup_step_runs_the_program_players_start(self):
        # RecompLauncher's guide (beta 68): "Ship one program, and keep it the one players start". AgeOfDecomp.exe
        # runs `cli prepare …` without a window; the AppImage's AppRun hands `cli …` to aod-cli inside it.
        m = release.manifest()
        self.assertEqual(m["prepare"]["program"], m["start"]["program"])
        self.assertEqual(m["prepare"]["program"][0], "AgeOfDecomp.exe")
        self.assertEqual(m["prepare"]["args"][:2], ["cli", "prepare"])

    def test_manifest_says_what_the_pc_needs_for_the_builds_we_ship(self):
        # RecompLauncher's `system` (SDK guide after beta 66): one part per system a release ships a build for. Linux's
        # glibc is the floor `release.py linux` checks every program of the AppImage against.
        self.assertEqual(release.manifest()["system"], {"windows": {"version": "10", "graphics": {"vulkan": "1.3"}},
                                                        "linux": {"glibc": "2.34", "graphics": {"vulkan": "1.3"}}})
        self.assertEqual(release.LINUX_GLIBC, (2, 34))

    def test_release_notes_join_the_changelogs_wrapped_lines(self):
        # GitHub keeps a release body's line breaks: `release.py notes` puts each list item and paragraph on one line.
        log = ("# Changelog\n\nIntro.\n\n**Not tested here:** no Radeon\nhere. Every release's notes say so.\n\n"
               "## 9.9.9 (2026-01-01)\n\nBuild again once. Setup\nkeeps your saves.\n\n### Fixes\n"
               "- One fix that is long\n  and wraps.\n- Another.\n\n## 9.9.8 (2025-12-31)\n- Old.\n")
        self.assertEqual(release.release_notes(log, "9.9.9"),
                         "Build again once. Setup keeps your saves.\n\n### Fixes\n- One fix that is long and wraps.\n"
                         "- Another.\n\n**Not tested here:** no Radeon here.\n")

    def test_every_release_says_what_is_not_tested_here(self):
        # Senpai has no Linux PC, SteamOS device, Radeon or PlayStation controller: every release's notes say so.
        log = (ROOT / "docs/CHANGELOG.md").read_text(encoding="utf-8")
        note = release.untested_note(log)
        for words in ("Linux", "SteamOS", "Radeon", "PlayStation", "game-*.log"):
            self.assertIn(words, note)
        self.assertTrue(release.release_notes(log, release.version()).rstrip("\n").endswith(note))
        with self.assertRaises(SystemExit):
            release.release_notes("# Changelog\n\n## 9.9.9 (2026-01-01)\n- A fix.\n", "9.9.9")

    def test_apprun_sends_the_headless_starts_to_aod_cli(self):
        # RecompLauncher's setup step (`cli prepare …`) and its start (`--play …`) must not need the launcher's window
        # (WebKitGTK): a Steam Deck has none. The AppRun hands both to aod-cli, which takes both (lib.rs headless).
        text = (ROOT / "packaging/linux/AppRun").read_text(encoding="utf-8")
        self.assertIn('if [ "$1" = "cli" ]; then\n    exec "$HERE/aod-cli" "$@"', text)
        self.assertIn('if [ "$arg" = "--play" ]; then\n        exec "$HERE/aod-cli" "$@"', text)
        self.assertNotIn("APPIMAGE", text.split("\n", 3)[3])   # RecompLauncher never sets it
        cli = (ROOT / "apps/launcher/src-tauri/src/bin/aod-cli.rs").read_text(encoding="utf-8")
        self.assertIn("aod_launcher_lib::headless(&args)", cli)

    def test_linux_tarball_is_the_folder_under_one_top_folder(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            folder = root / "AppDir"
            folder.mkdir()
            run = folder / "AppRun"
            run.write_text("#!/bin/sh\n", encoding="utf-8")
            run.chmod(0o755)
            (folder / "engine").mkdir()
            (folder / "engine/bundle.json").write_text("{}", encoding="utf-8")
            tarball = root / "AgeOfDecomp-v0-Linux-x64.tar.xz"
            release.pack_tarball(folder, tarball, "AgeOfDecomp-v0-Linux-x64")
            with tarfile.open(tarball) as t:
                members = {m.name: m for m in t.getmembers()}
            self.assertEqual(set(members), {"AgeOfDecomp-v0-Linux-x64", "AgeOfDecomp-v0-Linux-x64/AppRun",
                                            "AgeOfDecomp-v0-Linux-x64/engine",
                                            "AgeOfDecomp-v0-Linux-x64/engine/bundle.json"})
            self.assertTrue(all(m.uid == 0 and m.uname == "root" for m in members.values()))
            if sys.platform != "win32":   # NTFS keeps no execute bit
                self.assertEqual(members["AgeOfDecomp-v0-Linux-x64/AppRun"].mode & 0o777, 0o755)

    def test_dirty_repository_requires_opt_in_and_is_recorded(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bundle_fixture(root, source_revision="engine-rev")
            def git(cmd, **kwargs):
                return SimpleNamespace(stdout=" M file\n" if "status" in cmd else "repo-rev\n")
            with patch.object(release.subprocess, "run", side_effect=git), \
                    patch.object(release, "stale_engine_files", return_value=[]):
                with self.assertRaises(SystemExit):
                    release.release_info(False, root)
                info = release.release_info(True, root)
            self.assertEqual(info["repository_revision"], "repo-rev")
            self.assertEqual(info["engine_source_revision"], "engine-rev")
            self.assertTrue(info["dirty"])

    def test_clean_repository_is_recorded_as_clean(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bundle_fixture(root, source_revision="engine-rev")
            def git(cmd, **kwargs):
                return SimpleNamespace(stdout="" if "status" in cmd else "repo-rev\n")
            with patch.object(release.subprocess, "run", side_effect=git), \
                    patch.object(release, "stale_engine_files", return_value=[]):
                info = release.release_info(False, root)
            self.assertFalse(info["dirty"])
            self.assertFalse(info["engine_dirty"])

    def test_a_clean_repository_does_not_ship_an_engine_staged_dirty_or_before_its_last_change(self):
        def git(cmd, **kwargs):
            return SimpleNamespace(stdout="" if "status" in cmd else "repo-rev\n")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            # staged with --allow-dirty, then the changes were committed: the tree is clean, the bundle is not
            bundle_fixture(root, source_revision="engine-rev", dirty=True)
            with patch.object(release.subprocess, "run", side_effect=git), \
                    patch.object(release, "stale_engine_files", return_value=[]):
                with self.assertRaises(SystemExit):
                    release.release_info(False, root)
                self.assertTrue(release.release_info(True, root)["engine_dirty"])
            # staged clean, then an engine file changed in a later commit
            bundle_fixture(root, source_revision="engine-rev")
            with patch.object(release.subprocess, "run", side_effect=git), \
                    patch.object(release, "stale_engine_files", return_value=["runtime/a.c"]):
                with self.assertRaises(SystemExit):
                    release.release_info(False, root)
                self.assertFalse(release.release_info(True, root)["dirty"])

    def test_stale_engine_files_compares_the_repository_with_the_bundle(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source, stage = root / "engine-source", root / "stage"
            (source / "runtime").mkdir(parents=True)
            (source / "runtime/a.c").write_text("int a;\n", encoding="utf-8")
            (source / "runtime/b.c").write_text("int b;\n", encoding="utf-8")
            (source / "runtime/c.c").write_text("int c;\n", encoding="utf-8")
            (source / "VERSION").write_text("dev\n", encoding="utf-8")
            stage.mkdir()
            files = {"source/runtime/a.c": engine.sha256(source / "runtime/a.c"), "source/runtime/b.c": "0" * 64,
                     "source/VERSION": "another"}
            (stage / "bundle.json").write_text(json.dumps({"files": files}), encoding="utf-8")
            tracked = ["VERSION", "runtime/a.c", "runtime/b.c", "runtime/c.c"]
            with patch.object(engine, "ENGINE", source), patch.object(engine, "source_files", return_value=tracked):
                # b.c changed since the stage and c.c is new: both are named; VERSION is the bundle's own
                self.assertEqual(release.stale_engine_files(stage), ["runtime/b.c", "runtime/c.c"])

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



class ContentGuardTests(unittest.TestCase):
    def test_build_folders_are_refused_at_the_top_and_in_the_engine(self):
        # A name alone (no blob): only the path rules run.
        for name in ("work/game/main_0001.c", "out/release/a.zip", "engine/source/work/build/hwaoi.ini",
                     "engine/source/generated/main.c", "engine/source/game/hwaoi.exe.txt",
                     "engine/source/toolchain/bin/x"):
            self.assertIn("file inside an ignored build/dump folder", check_content.problems(name, None, None), name)
        for name in ("engine/source/runtime/vk_rec.c", "engine/source/tools/hwaoi.py", "docs/CHANGELOG.md"):
            self.assertEqual(check_content.problems(name, None, None), [], name)


if __name__ == "__main__":
    unittest.main()
