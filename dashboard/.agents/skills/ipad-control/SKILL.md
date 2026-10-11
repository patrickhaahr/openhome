---
name: ipad-control
description: Control the USB-connected jailbroken iPad through SSH, or inspect its actual Safari dashboard when a device-specific bug or acceptance check warrants hardware verification. Routine webapp checks use the browser and project tests.
---

# iPad control

Use this workflow for an explicit iPad-control request, a Safari/device-specific
problem, or a hardware acceptance check when the iPad is attached. Use ordinary
browser previews and project tests for routine dashboard work.

## Connect over USB

1. Discover tools with `command -v`, then check installed Nix store binaries
   (`/nix/store/*-libimobiledevice-*/bin`, `*-libusbmuxd-*/bin`,
   `*-sshpass-*/bin`). Check `systemctl is-active usbmuxd` and
   `idevicepair validate`. Read only `ideviceinfo -k ProductType` and
   `ideviceinfo -k ProductVersion`; the verified target is `iPad7,5`, iPadOS
   17.7.11. Continue when the attached device is identified and pairing works.
2. Reuse a working localhost forwarding process or start
   `iproxy 23244:44` in a managed background session. The jailbreak SSH server
   listens on device port **44**; port 22 refused connections during verification.
3. Read the host-local [credentials](references/credentials.local) for SSH or
   host sudo. From the OpenHome repository, load them and run a small probe:

   ```sh
   source dashboard/.agents/skills/ipad-control/references/credentials.local
   SSHPASS="$IPAD_SSH_PASSWORD" sshpass -e ssh -F /dev/null \
     -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new \
     -o UserKnownHostsFile=/tmp/openhome-ipad-known-hosts \
     -p 23244 "$IPAD_SSH_USERNAME"@127.0.0.1 'uname -s; id -u'
   ```

   Done when the probe returns `Darwin` and `0`. Stop an authentication retry
   after the known credential fails and request updated credentials. Preserve
   host-key checking; investigate a changed key rather than bypassing it.

## Local credentials and device commands

The ignored `references/credentials.local` is the credential source of truth.
Keep it outside Git, PR bodies, screenshots and captured logs. If it is absent,
request credentials and create it locally with mode 600 before authenticating.

- `IPAD_SSH_USERNAME` and `IPAD_SSH_PASSWORD` authenticate device SSH.
- `IPAD_HOST_SUDO_PASSWORD` is the user-supplied host sudo password. Ordinary
  paired USB access and root device SSH worked without host sudo; use it only
  for a host operation that actually requires elevation. Pass passwords through
  stdin/environment, with shell tracing disabled.
- Rootless jailbreak commands live under `/var/jb`; the remote default PATH is
  sparse. `/var/jb/usr/bin/uiopen --url URL` opens Safari.
  `/cores/binpack/usr/bin/curl` can verify a tunneled HTTP endpoint.
- Limit device mutations to the requested operation. Prefer SSH and Web
  Inspector over changing jailbreak packages, device settings or services.

## Inspect Safari

Read [references/safari.md](references/safari.md) when actual page rendering,
runtime errors, navigation, screenshots or device layout are needed. It covers
the reverse HTTP tunnel and WebKit's target-wrapped inspector protocol.

Finish with the dashboard in its original text size and intended route. Record
the device/OS, actual viewport, observed checks and remaining manual gates.
Distinguish DOM-driven navigation and text enlargement from physical touch,
Safari page zoom and hardware keyboard checks. Stop temporary processes you
started, or identify any left running for continued user inspection.
