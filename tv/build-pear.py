"""Build Pear locally from the backed-up firmware, without touching the TV."""
from pathlib import Path
import shutil
import sys
import tarfile

ROOT = Path(__file__).resolve().parent
UPSTREAM = ROOT / "upstream" / "PearWebOS"
sys.path.insert(0, str(UPSTREAM / "installer"))
import patch_home

build = ROOT / "private" / "pear-build"
stock = ROOT / "private" / "stock"
stock.mkdir(parents=True, exist_ok=True)
with tarfile.open(ROOT / "private" / "baseline" / "backup.tar") as backup:
    for rel in patch_home.NEEDED:
        member = backup.getmember("usr/palm/applications/com.webos.app.home/qml/" + rel)
        dest = stock / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(backup.extractfile(member).read())
if build.exists():
    shutil.rmtree(build)
build.mkdir()
patch_home.build(str(stock), str(build / "files"), str(ROOT / "launcher" / "OpenHome.qml"), {})
for directory in ("ui", "fonts"):
    shutil.copytree(ROOT / "launcher" / directory, build / directory)
shutil.copytree(ROOT / "private" / "icons", build / "ui" / "icons")
shutil.copy(ROOT / "launcher" / "LICENSE-PearWebOS", build)
shutil.copy(ROOT / "launcher" / "LICENSE-Phosphor", build)
# Preserve current power/firmware settings. Initially no Live-TV takeover daemon.
(build / "pear.conf").write_text("GUARD=0\nBOOT_TO_HOME=0\nQUICK_START=0\nNO_UPDATE_PROMPT=0\n")
for name in ("pearwebos", "homeguard", "luna-send-n", "tvkey.js"):
    dest = build / name
    content = (UPSTREAM / "tv" / name).read_text()
    if name == "homeguard":
        # Upstream kills every client's foreground subscription, even GUARD=0.
        # Our guard stays disabled; never terminate unrelated subscribers.
        lines = content.splitlines(keepends=True)
        matches = [line for line in lines if "kill $(ps" in line]
        if len(matches) != 1:
            raise SystemExit("Review changed upstream homeguard cleanup")
        content = content.replace(matches[0], "")
    dest.write_text(content)
    dest.chmod(0o755)
with tarfile.open(ROOT / "private" / "pear-install.tar", "w") as package:
    for item in sorted(build.iterdir()):
        def stable_time(info):
            info.mtime = 1700000000
            return info
        package.add(item, arcname=item.name, filter=stable_time)
print("PASS: firmware patches matched; native OpenHome launcher package built locally.")
print("Power settings, update settings, and Live TV are preserved.")
