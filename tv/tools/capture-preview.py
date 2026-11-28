"""Capture the actual TV UI with synthetic health/weather for public Git evidence."""
import shutil
import subprocess
import sys
import time

import preview_bridge
from tv_paths import TV_ROOT as root

remote = preview_bridge.remote


def keys(*names):
    remote("node /var/lib/pearwebos/tvkey.js " + " ".join(names))
    time.sleep(2)


def capture(name):
    subprocess.run([sys.executable, str(root / "tools" / "screen.py"), "public-" + name], check=True)
    shutil.copy(root / "private" / ("public-" + name + ".jpg"), root / "screenshots" / (name + ".jpg"))


preview_bridge.upload("preview", (root / "bridge/preview.js").read_bytes())
try:
    preview_bridge.launch("preview")
    time.sleep(2)
    remote("/var/lib/pearwebos/pearwebos on")
    capture("openhome")
    keys("DOWN")
    capture("openhome-apps")
    keys("BACK", "UP", "OK")
    capture("ember-idle")
    keys("OK")
    capture("ember-speaking")
finally:
    preview_bridge.restore("preview")
print("Public screenshots use synthetic values; the TV is back on the real API.")
