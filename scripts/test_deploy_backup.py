"""A pre-release app.db and its photos restore together, including the first nutrition release."""
import importlib.util
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("deploy_pi", Path(__file__).with_name("deploy-pi.py"))
deploy_pi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deploy_pi)


class DatabasePhotoBackup(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.data = self.root / "data"
        self.data.mkdir()
        self.backup = self.root / "backup"
        self.backup.mkdir()
        for name in ("app.db", "noop.db"):
            with sqlite3.connect(self.data / name) as database:
                database.execute("CREATE TABLE example (value TEXT)")
                database.execute("INSERT INTO example VALUES ('before release')")
        self.photo = "edc9bf88-4e5f-413c-a830-0acbd49469d9"
        self.stopped = False

    def docker(self, *args, **kwargs):
        if args == ("docker", "stop", "--time", "30", "openhome-api"):
            self.stopped = True
        elif args == ("docker", "start", "openhome-api"):
            self.assertTrue(self.stopped)
            self.stopped = False
            # A resumed API may replace/delete originals: the backup must already own its copy.
            shutil.rmtree(self.data / "meal-photos", ignore_errors=True)
        else:
            self.fail(f"Unexpected command {args}")

    def log_photo(self):
        with sqlite3.connect(self.data / "app.db") as database:
            database.execute("CREATE TABLE meal_logs (photo_id TEXT)")
            database.execute("INSERT INTO meal_logs VALUES (?)", (self.photo,))
        photos = self.data / "meal-photos"
        photos.mkdir()
        for suffix in (".avif", ".thumbnail.avif"):
            (photos / f"{self.photo}{suffix}").write_bytes(b"stored photo" + suffix.encode())

    def test_database_and_every_referenced_photo_restore_after_originals_are_replaced(self):
        self.log_photo()
        # The data copies must take place inside the stopped window, including sqlite's backup.
        copy = shutil.copy2
        def copy_while_stopped(source, destination):
            self.assertTrue(self.stopped)
            return copy(source, destination)
        with patch.object(deploy_pi, "run", self.docker), patch.object(deploy_pi.shutil, "copy2", copy_while_stopped):
            deploy_pi.back_up_data(self.data, self.backup)
        self.assertFalse(self.stopped)
        restored = self.root / "restored"
        shutil.copytree(self.backup, restored)
        with sqlite3.connect(restored / "app.db") as database:
            named = database.execute("SELECT photo_id FROM meal_logs").fetchone()[0]
            self.assertEqual(database.execute("SELECT value FROM example").fetchone()[0], "before release")
        for suffix in (".avif", ".thumbnail.avif"):
            self.assertEqual((restored / "meal-photos" / f"{named}{suffix}").read_bytes(), b"stored photo" + suffix.encode())

    def test_schema_before_nutrition_can_be_backed_up(self):
        with patch.object(deploy_pi, "run", self.docker):
            deploy_pi.back_up_data(self.data, self.backup)
        self.assertFalse(self.stopped)
        self.assertEqual(list((self.backup / "meal-photos").iterdir()), [])

    def test_missing_referenced_photo_aborts_and_resumes_previous_api(self):
        self.log_photo()
        (self.data / "meal-photos" / f"{self.photo}.thumbnail.avif").unlink()
        with patch.object(deploy_pi, "run", self.docker), self.assertRaises(FileNotFoundError):
            deploy_pi.back_up_data(self.data, self.backup)
        self.assertFalse(self.stopped)
