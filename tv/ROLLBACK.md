# TV rollback

## Restore the original Pear design while keeping privacy/Alexa protection

```sh
devenv shell -- just tv-ssh 'touch /var/lib/openhome-tv/client-disabled; /var/lib/openhome-tv/client-service.sh stop; /var/lib/pearwebos/pearwebos unmount; cp -a /var/lib/pearwebos-baseline/. /var/lib/pearwebos/; /var/lib/pearwebos/pearwebos on'
```

The baseline is the working photo-based Pear installation saved before the first
OpenHome deployment, also recorded in Git commit `5448646`. Extra OpenHome QML assets
are unused after restoring Pear's three mounted files. The client boot hook respects
`client-disabled`; no API calls occur while disabled. This does not restore Alexa or
change the privacy protections. To re-enable the OpenHome design, remove that one
flag and run `tv-deploy` from this branch.

The API credential can be removed separately from `/var/lib/openhome-tv/client.json`
after disabling the client. Never put that file in a backup intended for publication.

Run commands from the OpenHome root through `devenv shell -- just tv-ssh`.
SSH uses a local control socket and does not install an authentication key on
the TV. After reboot, reconnect with the existing root password; the password
is not saved in this repository.

## Restore LG Home

```sh
devenv shell -- just tv-ssh '/var/lib/pearwebos/pearwebos off'
```

This unmounts Pear's three QML overrides, discards Home's runtime checkpoint
so it is regenerated from stock, and cold-starts LG Home. Pear records a
disabled flag so its boot hook stays off. It does not alter system partitions.

The task's scripts and private baseline are under `tv/`. Do not restore the
entire backup over the live TV: restore only the specific settings/scripts
that were changed. Original signed firmware files need no restoration.

If a startup customization breaks boot, Homebrew's existing failsafe opens an
emergency telnet shell and skips custom hooks. This fallback was confirmed in
the installed startup source, not triggered in a destructive test.

## Disable automatic changes before any rollback

```sh
devenv shell -- just tv-ssh 'touch /var/lib/openhome-tv/disabled; chmod -x /var/lib/webosbrew/init.d/40-openhome-tv'
```

Pear has its separate `60-pearwebos` hook; `pearwebos off` disables Pear via its
own flag. The repaired hosts hook replaces `10-block-lg-telemetry` and must be
restored separately below. The original Homebrew startup script is unchanged.

## Restore Alexa execution

```sh
devenv shell -- just tv-script tv/alexa-block.sh restore
```

This checks the source/target inode before unmounting each recorded Alexa
override, unlocks the two apps and asks LG's hub to rescan its service files.
Original firmware payloads and service definitions then reappear.
Disable `40-openhome-tv` as above before restoring. A full reboot with that hook disabled also drops the
runtime masks. No Amazon account or app data is erased.

## Restore promotions/privacy settings

Disable the task's boot hook first once installed, and stop its bounded
background ACR recheck (or reboot with the hook disabled). Then:

```sh
devenv shell -- just tv-script tv/privacy-settings.sh restore
devenv shell -- just tv-restore-settings
```

Only task-owned setting keys are restored from the baseline. The original
`/tmp/acr.xml` is backed up on the TV as `/var/lib/openhome-tv/acr.xml.orig`.
The runtime restore removes the task-owned read-only ACR bind before copying
that original back; it does not remove unrelated mounts.
Alexa's original `isAllowed=false` preload exclusion and the other preload
choices are unchanged; no preload restoration is needed.

## Restore hosts blocking

```sh
devenv shell -- just tv-script tv/privacy-hosts.sh --restore
devenv shell -- just tv-ssh 'cp -p /var/lib/openhome-tv/original-10-block-lg-telemetry /var/lib/webosbrew/init.d/10-block-lg-telemetry'
```

The script verifies the active mount ID and inode before removing its one
override; Homebrew's underlying OTA protection stays mounted. The original
telemetry boot hook is in the private baseline tar. If restoring that hook,
note that its ineffective mount-detection behavior will return too.

## Restore picture warmth or remove the custom YouTube repository

Only `whiteBalanceColorTemperature` was changed from `warm2` to `warm3` in the
current default-input SDR Cinema profile. Select that same input/profile and run:

```sh
devenv shell -- just tv-ssh "/var/lib/pearwebos/luna-send-n -n 1 luna://com.webos.settingsservice/setSystemSettings '{\"category\":\"picture\",\"settings\":{\"whiteBalanceColorTemperature\":\"warm2\"}}'"
```

This leaves the existing Reduce Blue Light switch and Warm 50 slider untouched.
The exact prior readback is in ignored `private/picture-before-warm3.json`.

The RF1705 repository can be removed from Homebrew Channel's Settings by selecting
its URL/trash icon. This does not uninstall YouTube. The old v1.1.7 application
folder backup is `private/youtube-legacy-1.1.7.tar.gz`; it is an emergency file
backup, not a redistributable IPK or a verified reinstall procedure.

## Restore the normal picture profile

The eye now switches picture profiles. To select Cinema (now the film baseline):

```sh
devenv shell -- just tv-ssh "/var/lib/pearwebos/luna-send-n -n 1 luna://com.webos.settingsservice/setSystemSettings '{\"category\":\"picture\",\"settings\":{\"pictureMode\":\"cinema\"}}'"
```

`private/picture-normal-preserved.json` contains the normal settings captured
before configuring Expert Dark Room; `private/picture-expert2-before.json` contains
its prior values. Restore only the backed-up target profile's `settings` object
while that profile and input are active. `private/picture-before-amber.json`
records Cinema's RGB gains before the earlier green−10/blue−30 adjustment;
restoring those two gains to0 removes that tint while retaining Warm3.

## Undo the Cinema baseline

`private/cinema-reference-before-getSystemSettings.json` contains the full Cinema
snapshot immediately before the reference-oriented tuning. The submitted key list
is in `private/cinema-reference-applied.json`. Select Cinema on the same default
SDR input first, then restore **only** the intersection of that submitted
`settings` key list with the before snapshot's `settings`. Do not send the full
snapshot: it includes read-only, unsupported and unrelated settings.

Restore the three submitted global keys separately from the same before snapshot:
`eyeComfortMode: on`, `energySaving: auto`, `energySavingModified: false`.
They affect all profiles/inputs.
Read back the restored keys and compare with the snapshot.

TruMotion's original app-context value was not captured before the change; it
was absent from an ordinary settings query. It was subsequently set to `off` and
verified with explicit Home/YouTube/Netflix app IDs. An exact pre-change TruMotion
restoration cannot be claimed from this backup. The ordinary LCD Cinema factory
default is `cinemaClear`; use LG's native TruMotion menu if choosing to restore
that factory behaviour. This does not require a full picture-mode reset.

Expert Dark Room's stored profile controls were unchanged. Its independent
before/after snapshots are `private/cinema-reference-night-before.json` and
`private/cinema-reference-night-after.json`.

## Remove the red-button shortcut

The original Input Hook map is backed up locally in
`private/inputhook-before-picture-shortcut.json` and on the TV in
`/var/lib/openhome-tv/inputhook-before-picture-shortcut.json`. Red (398) was
unassigned. Remove only this task's binding, preserving any later shortcut edits:

```sh
devenv shell -- just tv-ssh 'node -' <<'JS'
var fs = require('fs');
var file = '/home/root/.config/lginputhook/keybinds.json';
var bindings = JSON.parse(fs.readFileSync(file, 'utf8'));
var command = '/usr/bin/flock -n /tmp/openhome-picture-toggle.lock /usr/bin/python /var/lib/openhome-tv/picture-toggle.py';
if (bindings['398'] && bindings['398'].command !== command) {
  throw new Error('Red now has another binding; leaving it alone');
}
delete bindings['398'];
fs.writeFileSync(file, JSON.stringify(bindings));
JS
```

Input Hook reloads the file automatically; a reboot is not required. The helper
can stay installed without a binding. No input service, app or boot hook needs
to be removed.
