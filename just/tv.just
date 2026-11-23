# Read or administer the LG TV through the task's authenticated SSH connection
[group('tv')]
[positional-arguments]
tv-ssh *args:
    sh tv/tools/ssh.sh "$@"

# Reconnect the task's SSH control socket after a TV reboot (prompts for password)
[group('tv')]
tv-connect:
    sh tv/tools/connect.sh

# Save a private, local backup of the LG TV before or after changes
[group('tv')]
tv-snapshot label:
    python3 tv/tools/snapshot.py {{quote(label)}}

# Capture the TV's actual display into the private evidence directory
[group('tv')]
tv-screen label:
    python3 tv/tools/screen.py {{quote(label)}}

# Install the locally checked Pear package without a boot hook
[group('tv')]
tv-install-pear: tv-check
    python3 tv/tools/install-pear.py

# Enable the reviewed Alexa/privacy and Pear boot hooks after live validation
[group('tv')]
tv-install-hooks: tv-check
    python3 tv/tools/install-hooks.py

# Run a reviewed shell script on the TV with optional arguments
[group('tv')]
tv-script file args='':
    sh tv/tools/ssh.sh 'sh -s -- {{args}}' < {{quote(file)}}

# Validate TV scripts and build Pear against the locally backed-up LG Home files
[group('tv')]
tv-check: tv-assets
    sh -n tv/tools/ssh.sh
    sh -n tv/runtime/alexa-block.sh
    sh -n tv/tools/probe.sh
    sh -n tv/runtime/privacy-settings.sh
    sh -n tv/runtime/privacy-hosts.sh
    sh -n tv/runtime/boot.sh
    sh -n tv/tools/connect.sh
    sh -n tv/runtime/client-service.sh
    node --check tv/bridge/client.js
    node --check tv/bridge/preview.js
    node --check tv/tools/verify-client.js
    node --check tv/launcher/ui/PictureProfileLifecycle.js
    python3 -c 'import ast; from pathlib import Path; [ast.parse(p.read_text(), filename=str(p)) for directory in ("tv/runtime", "tv/tools") for p in Path(directory).glob("*.py")]'
    node tv/bridge/test.js
    node tv/launcher/test-picture-profile.js
    python3 tv/tools/test-inputhook-startup.py
    qmlformat tv/launcher/OpenHome.qml tv/launcher/ui/*.qml > /dev/null
    python3 tv/tools/build-pear.py

# Render dashboard icons and agent vectors for LG's Qt PNG image loader
[group('tv')]
tv-assets:
    node tv/tools/export-icons.mjs

# Update OpenHome TV UI and its local client, preserving the original Pear UI
[group('tv')]
tv-deploy: tv-check
    python3 tv/tools/deploy-launcher.py

# Bind red to the native Cinema/night profile toggle without replacing other shortcuts
[group('tv')]
tv-picture-shortcut: tv-check
    python3 tv/tools/install-picture-shortcut.py

# Refresh Input Hook's elevated service registration before its normal boot startup
[group('tv')]
tv-inputhook-startup: tv-check
    python3 tv/tools/install-inputhook-startup.py

# Read-only acceptance checks from the TV's native Node runtime
[group('tv')]
tv-client-check:
    sh tv/tools/ssh.sh 'node -' < tv/tools/verify-client.js

# Save actual TV screenshots with synthetic health/weather, then restore live data
[group('tv')]
tv-preview-capture:
    python3 tv/tools/capture-preview.py

# Restore the exact setting values changed during TV setup
[group('tv')]
tv-restore-settings:
    python3 tv/tools/restore-settings.py

# Assert live Alexa blocking, privacy/preload settings, resolver and local control
[group('tv')]
tv-verify:
    python3 tv/tools/verify-live.py
