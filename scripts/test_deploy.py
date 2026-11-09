import contextlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest

from deploy import bump_versions, changed_areas, versions

REPOSITORY_LOCAL_VARIABLES = (
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_GRAFT_FILE",
    "GIT_NAMESPACE",
)


@contextlib.contextmanager
def metadata_inside_the_fixture_repository():
    """Resolve fixture Git metadata inside the fixture despite inherited repository-locator and index variables."""
    inherited = {name: os.environ.pop(name) for name in REPOSITORY_LOCAL_VARIABLES if name in os.environ}
    try:
        yield
    finally:
        os.environ.update(inherited)


class ReleaseVersions(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        for area in ("api", "mcp", "dashboard"):
            (self.root / area).mkdir()
        (self.root / "api/Cargo.toml").write_text('[package]\nname = "openhome-api"\nversion = "0.6.9"\n')
        (self.root / "mcp/Cargo.toml").write_text('[package]\nname = "openhome-mcp"\nversion = "0.3.1"\n')
        (self.root / "dashboard/package.json").write_text('{"name": "dashboard", "version": "0.2.2"}\n')
        self.api_lock = '[[package]]\nname = "openhome-api"\nversion = "0.6.9"\n'
        (self.root / "api/Cargo.lock").write_text(self.api_lock)
        (self.root / "mcp/Cargo.lock").write_text(self.api_lock + '\n[[package]]\nname = "openhome-mcp"\nversion = "0.3.1"\n')

    def test_patch_release_updates_manifests_and_path_dependency_locks(self):
        result = bump_versions(self.root)
        self.assertEqual(result, {"api": "0.6.10", "mcp": "0.3.2", "dashboard": "0.2.3"})
        self.assertEqual(versions(self.root), result)
        for area in ("api", "mcp"):
            packages = tomllib.loads((self.root / area / "Cargo.lock").read_text())["package"]
            for package in packages:
                self.assertEqual(package["version"], result[package["name"].removeprefix("openhome-")])
        self.assertEqual(json.loads((self.root / "dashboard/package.json").read_text())["name"], "dashboard")

    def test_dashboard_release_leaves_api_and_mcp_versions_alone(self):
        before = {path: path.read_bytes() for path in self.root.glob("[am]*/*")}
        result = bump_versions(self.root, ["dashboard"])
        self.assertEqual(result, {"api": "0.6.9", "mcp": "0.3.1", "dashboard": "0.2.3"})
        self.assertEqual({path: path.read_bytes() for path in before}, before)

    def test_api_release_brings_mcp_along(self):
        result = bump_versions(self.root, ["api"])
        self.assertEqual(result, {"api": "0.6.10", "mcp": "0.3.2", "dashboard": "0.2.2"})

    def test_changes_since_each_area_release_choose_the_areas(self):
        outer = tempfile.TemporaryDirectory()
        self.addCleanup(outer.cleanup)
        outer = Path(outer.name)
        git_in = lambda repository, *args: subprocess.run(
            ["git", *args], cwd=repository, check=True, capture_output=True,
            # The developer's global git hooks must not judge these fixture commits.
            env=os.environ | {"GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"},
        )
        with metadata_inside_the_fixture_repository():
            git_in(outer, "init", "-q")
            (outer / "dashboard").mkdir()
            (outer / "dashboard/released.css").write_text("released {}\n")
            git_in(outer, "add", ".")
            git_in(outer, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "outer release")
        outer_index = outer / ".git/index"
        outer_config = outer / ".git/config"
        before = (outer_index.read_bytes(), outer_config.read_bytes())
        os.environ["GIT_INDEX_FILE"] = str(outer_index)
        os.environ["GIT_DIR"] = str(outer / ".git")
        try:
            with metadata_inside_the_fixture_repository():
                git = lambda *args: git_in(self.root, *args)
                git("init", "-q")
                git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "root")
                git("add", ".")
                git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "release")
                self.assertEqual(changed_areas(self.root), [])
                (self.root / "dashboard/idle.css").write_text("a {}\n")
                self.assertEqual(changed_areas(self.root), ["dashboard"])
                (self.root / "api/main.rs").write_text("fn main() {}\n")
                self.assertEqual(changed_areas(self.root), ["api", "mcp", "dashboard"])
                self.assertNotEqual(git("rev-parse", "HEAD").stdout, git_in(outer, "rev-parse", "HEAD").stdout)
        finally:
            os.environ.pop("GIT_INDEX_FILE", None)
            os.environ.pop("GIT_DIR", None)
        self.assertEqual((outer_index.read_bytes(), outer_config.read_bytes()), before)

    def test_inconsistent_lock_aborts_before_changing_any_version(self):
        lock = self.root / "mcp/Cargo.lock"
        lock.write_text(lock.read_text().replace('version = "0.6.9"', 'version = "0.6.8"'))
        before = {path: path.read_bytes() for path in self.root.glob("*/*")}
        with self.assertRaises(ValueError):
            bump_versions(self.root)
        self.assertEqual({path: path.read_bytes() for path in before}, before)

    def test_failed_checks_keep_one_pending_release_across_retries(self):
        scripts = self.root / "scripts"
        scripts.mkdir()
        shutil.copy2(Path(__file__).with_name("deploy.py"), scripts / "deploy.py")
        binaries = self.root / "bin"
        binaries.mkdir()
        # SSH preflight and builder setup succeed; the quality gate is unavailable.
        for name, body in {
            "ssh": "exit 0",
            "just": 'case "$1" in docker-builder) exit 0 ;; *) exit 7 ;; esac',
        }.items():
            executable = binaries / name
            executable.write_text("#!/bin/sh\n" + body + "\n")
            executable.chmod(0o700)
        environment = {**os.environ, "PATH": str(binaries) + os.pathsep + os.environ["PATH"]}
        expected = {"api": "0.6.10", "mcp": "0.3.2", "dashboard": "0.2.3"}
        for mode in (["deploy", "api", "dashboard"], ["deploy", "api", "dashboard"], ["current"]):
            result = subprocess.run([sys.executable, str(scripts / "deploy.py"), *mode],
                                    cwd=self.root, env=environment, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(versions(self.root), expected)
            self.assertEqual(json.loads((self.root / ".devenv/release.json").read_text()),
                             {"versions": expected, "areas": ["api", "mcp", "dashboard"]})


if __name__ == "__main__":
    unittest.main()
