"""Bind the red key through the installed Input Hook, preserving other shortcuts."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent
ssh = ["sh", str(root / "ssh.sh")]
config = "/home/root/.config/lginputhook/keybinds.json"
binding = {
    "action": "exec",
    "command": "/usr/bin/flock -n /tmp/openhome-picture-toggle.lock /usr/bin/python /var/lib/openhome-tv/picture-toggle.py",
}
original = subprocess.run(ssh + ["cat " + config], capture_output=True, check=True).stdout
bindings = json.loads(original)
if not isinstance(bindings, dict) or ("398" in bindings and bindings["398"] != binding):
    raise SystemExit("The red button already has another binding; refusing to overwrite it")
backup = root / "private/inputhook-before-picture-shortcut.json"
if not backup.exists():
    with backup.open("xb") as file:
        file.write(original)
    backup.chmod(0o600)

# Write in place, like Input Hook's own UI, so its file watcher reloads the map.
script = """
var fs = require('fs');
var config = %s;
var original = fs.readFileSync(config);
var bindings = JSON.parse(original);
var binding = %s;
if (bindings['398'] && (bindings['398'].action !== binding.action || bindings['398'].command !== binding.command)) {
  throw new Error('The red binding changed during installation');
}
var backup = '/var/lib/openhome-tv/inputhook-before-picture-shortcut.json';
if (!fs.existsSync(backup)) fs.writeFileSync(backup, original, {mode: 384, flag: 'wx'});
fs.writeFileSync('/var/lib/openhome-tv/picture-toggle.py', %s, {mode: 420});
bindings['398'] = binding;
fs.writeFileSync(config, JSON.stringify(bindings));
console.log('Red button (398) bound; other Input Hook bindings preserved.');
""" % (json.dumps(config), json.dumps(binding), json.dumps((root / "picture-toggle.py").read_text()))
subprocess.run(ssh + ["node -"], input=script.encode(), check=True)
