"""Start or stop the Demo data bridge on the TV: mock recovery for today, everything else through the real client."""
import json
import sys

import preview_bridge
from tv_paths import TV_ROOT as root

SNAPSHOT = root.parent / "dashboard/src/lib/demo/snapshot.json"
DATA = preview_bridge.script_path("demo").replace(".js", ".json")


def capture_day_recovery():
    snapshot = json.loads(SNAPSHOT.read_text())
    captured_on = snapshot["capturedOn"]
    recovery = snapshot["responses"]["/api/training/days/%s/recovery?" % captured_on]
    return json.dumps({"capturedOn": captured_on, "recovery": recovery}).encode()


def start():
    files = [(DATA, capture_day_recovery())]
    preview_bridge.upload("demo", (root / "bridge/demo.js").read_bytes(), files)
    try:
        preview_bridge.launch("demo")
    except BaseException:
        preview_bridge.restore("demo", files)
        raise
    print("Demo bridge running: today's recovery is mocked; the TV client is stopped until tv-demo-stop.")


def stop():
    preview_bridge.restore("demo", [(DATA, b"")])
    print("Real TV client restored.")


actions = {"start": start, "stop": stop}
if len(sys.argv) != 2 or sys.argv[1] not in actions:
    sys.exit("usage: demo-bridge.py start|stop")
actions[sys.argv[1]]()
