"""Capture TV state privately; never print settings or app credentials."""
import json
import subprocess
import sys

from tv_paths import SSH, TV_ROOT as ROOT

label = sys.argv[1]
if not label.replace("-", "").isalnum():
    raise SystemExit("Use an alphanumeric snapshot label")
out = ROOT / "private" / label
out.mkdir(mode=0o700, parents=True, exist_ok=False)


def capture(name, command):
    result = subprocess.run(SSH + [command], capture_output=True, check=True)
    (out / name).write_bytes(result.stdout)


def luna(name, uri, payload="{}"):
    capture(name, 'LS=/var/lib/pearwebos/luna-send-n; [ -x "$LS" ] || LS=/tmp/openhome-luna; '
            + '"$LS" -n 1 ' + uri + " '" + payload + "'")


capture("system.txt", "id; cat /etc/os-release; cat /etc/starfish-release; cat /proc/uptime; cat /proc/sys/kernel/random/boot_id")
capture("mounts.txt", "cat /proc/mounts")
capture("processes.txt", "ps -eo pid,args")
capture("input-devices.txt", "cat /proc/bus/input/devices")
capture("hosts.txt", "cat /etc/hosts")
luna("preload.json", "luna://com.webos.service.preloadmanager/getPreloadPolicy")
for method in ("listApps", "listLaunchPoints", "running", "getForegroundAppInfo"):
    luna(method + ".json", "luna://com.webos.applicationManager/" + method)
for category in ("general", "option", "support", "other"):
    payload = json.dumps({"category": category})
    luna("settings-" + category + ".json", "luna://com.webos.settingsservice/getSystemSettings", payload)
capture("backup.tar", "tar -C / -cf - var/lib/webosbrew var/preferences/webos-preload-manager-conf.json var/luna/preferences home/root/.config/lginputhook usr/palm/applications/com.webos.app.home/qml usr/palm/applications/amazon.alexa.view usr/palm/applications/amazon.alexapr")
print("Private snapshot saved:", out.relative_to(ROOT.parent))
