"""Install the checked local package; boot persistence is enabled after validation."""
import subprocess

from tv_paths import SSH, TV_ROOT as root

command = "test ! -e /var/lib/pearwebos && mkdir -p /var/lib/pearwebos && tar -C /var/lib/pearwebos -xf - && sh -n /var/lib/pearwebos/pearwebos && /var/lib/pearwebos/pearwebos on"
with (root / "private" / "pear-install.tar").open("rb") as package:
    subprocess.run(SSH + [command], stdin=package, check=True)
