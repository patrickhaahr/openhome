"""Toggle native picture profiles; compatible with the TV's Python 2.7 and Python 3."""
from __future__ import unicode_literals

import json
import subprocess
import sys


def settings(method, payload):
    response = json.loads(subprocess.check_output([
        "/var/lib/pearwebos/luna-send-n", "-n", "1",
        "luna://com.webos.settingsservice/" + method, json.dumps(payload),
    ]))
    if not response.get("returnValue"):
        raise ValueError(response.get("errorText", "Picture service rejected request"))
    return response


def read():
    response = settings("getSystemSettings", {"category": "picture", "keys": ["pictureMode"]})
    dimension = response.get("dimension", {})
    if (dimension.get("input") != "default" or dimension.get("dynamicRange") != "sdr"
            or dimension.get("_3dStatus") != "2d"):
        raise ValueError("The Cinema/night profiles are configured for default-input 2D SDR only")
    mode = response.get("settings", {}).get("pictureMode")
    # JSON strings and these literals are Unicode in both supported runtimes.
    if not isinstance(mode, type("")) or not mode:
        raise ValueError("Missing current picture mode")
    return mode


try:
    target = "cinema" if read() == "expert2" else "expert2"
    settings("setSystemSettings", {"category": "picture", "settings": {"pictureMode": target}})
    if read() != target:
        raise ValueError("Picture mode did not match requested profile")
    print("Picture profile: " + target)
except (ValueError, subprocess.CalledProcessError, OSError) as error:
    sys.stderr.write("Picture shortcut: %s\n" % error)
    sys.exit(1)
