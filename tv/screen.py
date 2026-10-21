"""Save a real TV screenshot in the ignored private directory."""
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parent
name = sys.argv[1]
if not name.replace("-", "").isalnum():
    raise SystemExit("Use an alphanumeric screenshot name")
payload = json.dumps({"path": "/tmp/openhome-tv.jpg", "method": "DISPLAY", "width": 1920, "height": 1080, "format": "JPEG"})
command = 'LS=/var/lib/pearwebos/luna-send-n; [ -x "$LS" ] || LS=/tmp/openhome-luna; "$LS" -n 1 luna://com.webos.service.capture/executeOneShot ' + "'" + payload + "'"
result = subprocess.run(["sh", str(root / "ssh.sh"), command], capture_output=True, check=True)
reply = json.loads(result.stdout)
if not reply.get("returnValue"):
    raise SystemExit(reply)
image = subprocess.run(["sh", str(root / "ssh.sh"), "cat /tmp/openhome-tv.jpg"], capture_output=True, check=True)
dest = root / "private" / (name + ".jpg")
dest.write_bytes(image.stdout)
print(dest)
