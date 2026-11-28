"""Check the stand-in bridge start/restore commands without contacting the TV."""
import json
from pathlib import Path
import runpy
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import preview_bridge

DEMO_BRIDGE = Path(__file__).resolve().parent / "demo-bridge.py"


class StandIn(unittest.TestCase):
    def setUp(self):
        self.commands = []
        self.original = preview_bridge.remote
        preview_bridge.remote = lambda command, **kwargs: self.commands.append((command, kwargs.get("input")))

    def tearDown(self):
        preview_bridge.remote = self.original

    def run_demo(self, action):
        argv, sys.argv = sys.argv, [str(DEMO_BRIDGE), action]
        try:
            runpy.run_path(str(DEMO_BRIDGE), run_name="__main__")
        finally:
            sys.argv = argv

    def test_launch_stops_the_real_client_before_the_stand_in_listens(self):
        preview_bridge.launch("demo")
        command = self.commands[0][0]
        self.assertLess(command.index("client-disabled"), command.index("client-service.sh stop"))
        self.assertLess(command.index("client-service.sh stop"), command.index("node /tmp/openhome-tv-demo.js"))

    def test_restore_kills_only_its_own_bridge_then_starts_the_real_client(self):
        preview_bridge.restore("preview")
        command = self.commands[0][0]
        self.assertIn("'node /tmp/openhome-tv-preview.js '*)", command)
        self.assertLess(command.index("rm -f"), command.index("client-service.sh start"))
        self.assertTrue(command.endswith("/var/lib/pearwebos/pearwebos on"))

    def test_demo_start_uploads_the_script_and_the_capture_day_recovery(self):
        self.run_demo("start")
        uploads = {command: data for command, data in self.commands if command.startswith("cat > ")}
        self.assertIn("function createRequest", uploads["cat > /tmp/openhome-tv-demo.js"].decode())
        data = json.loads(uploads["cat > /tmp/openhome-tv-demo.json"])
        self.assertEqual(data["recovery"]["day"], data["capturedOn"])
        self.assertIn("touch /var/lib/openhome-tv/client-disabled", self.commands[-1][0])

    def test_demo_stop_removes_the_data_file_and_restores_the_client(self):
        self.run_demo("stop")
        command = self.commands[0][0]
        self.assertIn("/tmp/openhome-tv-demo.json", command)
        self.assertIn("client-service.sh start", command)


if __name__ == "__main__":
    unittest.main()
