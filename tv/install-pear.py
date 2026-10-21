"""Install the checked local package; boot persistence is enabled after validation."""
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent
command = "test ! -e /var/lib/pearwebos && mkdir -p /var/lib/pearwebos && tar -C /var/lib/pearwebos -xf - && sh -n /var/lib/pearwebos/pearwebos && /var/lib/pearwebos/pearwebos on"
with (root / "private" / "pear-install.tar").open("rb") as package:
    subprocess.run(["sh", str(root / "ssh.sh"), command], stdin=package, check=True)
