"""Capture the actual TV UI with synthetic health/weather for public Git evidence."""
from pathlib import Path
import shutil
import subprocess
import sys
import time

root = Path(__file__).resolve().parent
ssh = ["sh", str(root / "ssh.sh")]


def remote(command, **kwargs):
    return subprocess.run(ssh + [command], check=True, **kwargs)


def keys(*names):
    remote("node /var/lib/pearwebos/tvkey.js " + " ".join(names))
    time.sleep(2)


def capture(name):
    subprocess.run([sys.executable, str(root / "screen.py"), "public-" + name], check=True)
    shutil.copy(root / "private" / ("public-" + name + ".jpg"), root / "screenshots" / (name + ".jpg"))


remote("cat > /tmp/openhome-tv-preview.js", input=(root / "bridge/preview.js").read_bytes())
try:
    remote("touch /var/lib/openhome-tv/client-disabled && /var/lib/openhome-tv/client-service.sh stop && { setsid node /tmp/openhome-tv-preview.js </dev/null >/tmp/openhome-tv-preview.log 2>&1 & }")
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
    # Match the exact fixture command; never terminate another Node client.
    remote("for p in /proc/[0-9]*/cmdline; do [ -r \"$p\" ] || continue; command=$(tr '\\000' ' ' < \"$p\" 2>/dev/null) || continue; case \"$command\" in 'node /tmp/openhome-tv-preview.js '*) pid=${p#/proc/}; kill \"${pid%/cmdline}\";; esac; done; rm -f /tmp/openhome-tv-preview.js /tmp/openhome-tv-preview.log /var/lib/openhome-tv/client-disabled; /var/lib/openhome-tv/client-service.sh start; /var/lib/pearwebos/pearwebos on")
print("Public screenshots use synthetic values; the TV is back on the real API.")
