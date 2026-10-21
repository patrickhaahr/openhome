"""Update the TV UI/client, preserving the working Pear installation for rollback."""
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile

from tv_paths import SSH, TV_ROOT as root

# Read the CLI's existing credential without ever printing it or writing it to Git.
key_file = Path(os.environ.get("OPENHOME_API_KEY_FILE", Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "openhome/api-key"))
key = os.environ.get("OPENHOME_API_KEY") or key_file.read_text().strip()
if not key or any(c.isspace() for c in key):
    raise SystemExit("A valid existing OpenHome CLI API key is required")
config = json.dumps({"apiKey": key, "transportHost": "100.120.132.42", "serverName": "openhome.haahr.me"}).encode()
files = {
    "var/lib/openhome-tv/client.js": ((root / "bridge/client.js").read_bytes(), 0o644),
    "var/lib/openhome-tv/client-service.sh": ((root / "runtime" / "client-service.sh").read_bytes(), 0o755),
    "var/lib/openhome-tv/picture-toggle.py": ((root / "runtime" / "picture-toggle.py").read_bytes(), 0o644),
    "var/lib/openhome-tv/client.json": (config, 0o600),
    "var/lib/webosbrew/init.d/45-openhome-client": (b'#!/bin/sh\n/var/lib/openhome-tv/client-service.sh start\n', 0o755),
}
buffer = io.BytesIO()
with tarfile.open(fileobj=buffer, mode="w") as archive:
    for name, (content, mode) in files.items():
        info = tarfile.TarInfo(name); info.size = len(content); info.mode = mode; info.mtime = 1700000000
        archive.addfile(info, io.BytesIO(content))
# Backup once, before touching any current UI. The API key is piped in-memory over SSH.
subprocess.run(SSH + ["test -d /var/lib/pearwebos && { test -d /var/lib/pearwebos-baseline || cp -a /var/lib/pearwebos /var/lib/pearwebos-baseline; } && { test ! -x /var/lib/openhome-tv/client-service.sh || /var/lib/openhome-tv/client-service.sh stop; } && umask 077 && tar -C / -xf -"], input=buffer.getvalue(), check=True)
with (root / "private/pear-install.tar").open("rb") as package:
    subprocess.run(SSH + ["/var/lib/pearwebos/pearwebos unmount && tar -C /var/lib/pearwebos -xf - && /var/lib/openhome-tv/client-service.sh start && /var/lib/pearwebos/pearwebos on"], stdin=package, check=True)
print("OpenHome launcher installed. Pear baseline retained on TV; API credential is root-only.")
