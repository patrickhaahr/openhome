"""Refresh Input Hook's elevated developer-service registration before boot activation."""
import json
import subprocess

from tv_paths import SSH, TV_ROOT

source = (TV_ROOT / "runtime/inputhook-start.py").read_text()
hook = "#!/bin/sh\nexec /usr/bin/python /var/lib/openhome-tv/inputhook-start.py\n"
script = """
var fs = require('fs');
var hook = '/var/lib/webosbrew/init.d/50-openhome-inputhook';
var expected = %s;
if (!fs.existsSync('/media/developer/apps/usr/palm/services/org.webosbrew.inputhook.service/service.js')) {
  throw new Error('Install and elevate LG Input Hook first');
}
if (fs.existsSync(hook) && fs.readFileSync(hook, 'utf8') !== expected) {
  throw new Error('The Input Hook startup path already has another owner');
}
function write(file, content) {
  fs.writeFileSync(file + '.tmp', content, {mode: 493});
  fs.chmodSync(file + '.tmp', 493);
  fs.renameSync(file + '.tmp', file);
}
write('/var/lib/openhome-tv/inputhook-start.py', %s);
write(hook, expected);
console.log('Installed bounded Input Hook startup; key mappings unchanged.');
""" % (json.dumps(hook), json.dumps(source))
subprocess.run(SSH + ["node -"], input=script.encode(), check=True)
subprocess.run(SSH + ["/var/lib/webosbrew/init.d/50-openhome-inputhook"], check=True)
