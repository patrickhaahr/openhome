"""Exercise the boot script against the observed LS2 volatile-cache contract."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parent.parent / "runtime/inputhook-start.py"
if len(sys.argv) > 1 and Path(sys.argv[1]).is_file():
    SCRIPT = Path(sys.argv.pop(1)).resolve()

# Model the platform boundary: normal rescans retain cached developer-service
# definitions; volatile rescans load the already-elevated definition from disk.
PLATFORM = '''#!/usr/bin/env python3
import os
from pathlib import Path
import sys
import time

directory = Path(os.environ["TV_TEST_STATE"])
name = Path(sys.argv[0]).name
mode = os.environ.get("TV_TEST_FAILURE", "")
if name == "ls-control":
    if mode == "registry":
        sys.exit(7)
    if sys.argv[1:] == ["scan-volatile-dirs"]:
        (directory / "cached-launcher").write_text("root")
elif name in ("luna-send", "luna-send-n"):
    if mode == "activation":
        time.sleep(30)
    if (directory / "cached-launcher").read_text() != "root":
        print('{"returnValue":false,"reason":"sandboxed"}')
        sys.exit(0)
    (directory / "active-service").write_text("root")
    print('{"returnValue":true,"response":"Started."}')
else:
    sys.exit(2)
'''


class InputHookStartupTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        (self.directory / "cached-launcher").write_text("sandboxed")
        executable = self.directory / "platform"
        executable.write_text(PLATFORM)
        executable.chmod(0o755)
        for name in ("ls-control", "luna-send", "luna-send-n"):
            (self.directory / name).symlink_to(executable)
        self.environment = dict(os.environ, TV_TEST_STATE=str(self.directory),
                                PATH=str(self.directory) + os.pathsep + os.environ["PATH"])

    def run_startup(self, failure=""):
        interpreter = sys.executable if SCRIPT.suffix == ".py" else "sh"
        return subprocess.run([interpreter, str(SCRIPT)], capture_output=True, text=True,
                              env=dict(self.environment, TV_TEST_FAILURE=failure), timeout=15)

    def test_boot_activates_elevated_service_instead_of_cached_sandbox(self):
        result = self.run_startup()
        self.assertEqual(result.returncode, 0, result.stderr)
        service = self.directory / "active-service"
        self.assertTrue(service.exists(), "Input Hook never started outside the cached sandbox")
        self.assertEqual(service.read_text(), "root")

    def test_failed_registry_refresh_does_not_activate_stale_service(self):
        result = self.run_startup("registry")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.directory / "active-service").exists())

    def test_unresponsive_activation_does_not_block_boot(self):
        result = self.run_startup("activation")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.directory / "active-service").exists())


if __name__ == "__main__":
    unittest.main()
