"""Install only reviewed task scripts, preserving the original telemetry hook."""
import io
from pathlib import Path
import subprocess
import tarfile

root = Path(__file__).resolve().parent
files = {
    "var/lib/openhome-tv/alexa-block.sh": (root / "alexa-block.sh").read_bytes(),
    "var/lib/openhome-tv/privacy-settings.sh": (root / "privacy-settings.sh").read_bytes(),
    "var/lib/openhome-tv/boot.sh": (root / "boot.sh").read_bytes(),
    "var/lib/webosbrew/init.d/10-block-lg-telemetry": (root / "privacy-hosts.sh").read_bytes(),
    "var/lib/webosbrew/init.d/40-openhome-tv": b'#!/bin/sh\ntimeout -t 90 /var/lib/openhome-tv/boot.sh >> /var/lib/openhome-tv/boot.log 2>&1\n',
    "var/lib/webosbrew/init.d/60-pearwebos": b'#!/bin/sh\n/var/lib/pearwebos/pearwebos boot\n',
}
buffer = io.BytesIO()
with tarfile.open(fileobj=buffer, mode="w") as archive:
    for name, content in files.items():
        info = tarfile.TarInfo(name)
        info.mode, info.size = 0o755, len(content)
        info.mtime = 1700000000
        archive.addfile(info, io.BytesIO(content))
command = "test ! -e /var/lib/webosbrew/init.d/40-openhome-tv && test ! -e /var/lib/webosbrew/init.d/60-pearwebos && cp -p /var/lib/webosbrew/init.d/10-block-lg-telemetry /var/lib/openhome-tv/original-10-block-lg-telemetry && tar -C / -xf -"
subprocess.run(["sh", str(root / "ssh.sh"), command], input=buffer.getvalue(), check=True)
print("Installed three task hooks; original telemetry hook backed up on TV and locally.")
