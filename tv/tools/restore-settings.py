"""Restore only settings changed by this task from the private baseline."""
import json
import shlex
import subprocess

from tv_paths import SSH, TV_ROOT as root

keys = {
    "general": ["homePromotion", "sportsAlarm"],
    "option": ["livePromotion", "webOSPromotionVideo", "promotionStreamer", "livePlus", "watchedListCollection", "dbgLogUpload", "faultLogUpload"],
    "other": ["contentRecommendation"],
}
for category, names in keys.items():
    baseline = json.loads((root / "private" / "baseline" / f"settings-{category}.json").read_text())["settings"]
    payload = json.dumps({"category": category, "settings": {key: baseline[key] for key in names if key in baseline}})
    command = "/var/lib/pearwebos/luna-send-n -n 1 luna://com.webos.settingsservice/setSystemSettings " + shlex.quote(payload)
    result = subprocess.run(SSH + [command], capture_output=True, check=True)
    if not json.loads(result.stdout).get("returnValue"):
        raise SystemExit("Settings restore failed for " + category)
print("Task-owned settings restored from baseline.")
