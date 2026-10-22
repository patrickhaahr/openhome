"""Refresh Input Hook's boot registration; supports the TV's Python 2.7."""
from __future__ import print_function

import json
import os
import subprocess
import sys
import time

# LG's luna-send can exit without a reply with closed stdin. Pear's native
# client handles noninteractive LS2 calls reliably; keep it ahead of system tools.
os.environ["PATH"] = "/var/lib/pearwebos:" + os.environ.get("PATH", "/usr/bin:/bin")


def run(command):
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    deadline = time.time() + 10
    while process.poll() is None:
        if time.time() >= deadline:
            process.kill()
            process.communicate()
            raise RuntimeError(command[0] + " timed out")
        time.sleep(0.05)
    output, error = process.communicate()
    if process.returncode:
        raise RuntimeError(command[0] + " failed: " + error.decode("utf-8", "replace"))
    return output


try:
    # scan-services leaves cached developer-service launchers unchanged on this TV.
    run(["ls-control", "scan-volatile-dirs"])
    reply = json.loads(run(["luna-send-n", "-n", "1",
                           "luna://org.webosbrew.inputhook.service/start", "{}"]))
    if reply.get("returnValue") is not True:
        raise ValueError("Input Hook did not acknowledge startup")
    print("Input Hook elevated registration refreshed and service started.")
except (OSError, RuntimeError, ValueError) as error:
    sys.stderr.write("Input Hook startup: %s\n" % error)
    sys.exit(1)
