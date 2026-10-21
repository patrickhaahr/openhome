# LG TV work log

## Scope

Approved: install and verify Pear, hide Sports Alert, Home Dashboard, Media
Player and Alexa; prevent Alexa execution; selectively disable promotions,
ACR and unwanted preloads; repair telemetry blocking. Keep normal app updates,
HDMI, playback, SSH, casting and local OpenHome/SSAP control available.

Step 9 is a later OpenHome integration into a modified Pear launcher. No
OpenHome product changes belong in steps 1–8.

## Device and baseline

- LG 55NANO806PA; webOS 6.5.3, firmware 03.53.45; root SSH verified.
- Homebrew Channel 0.7.3, LG Input Hook 1.4.0; existing remote shortcuts preserved.
- Private backup: `tv/private/baseline/backup.tar` (69 MiB, 1,191 entries),
  including LG Home QML, Alexa files, settings and Homebrew scripts.
- Settings and process/mount/app inventories are in the same ignored directory.
  These contain personal state and must not be committed.
- Live Plus/ACR was already off; Home promotions and Sports Alert were on.
- Quick Start+ and full Home autolaunch were already enabled. Power-on screen
  was `todays`; unrelated power choices are initially preserved.
- Alexa companion preloading was already disallowed. The Alexa apps remain
  launchable until the execution-blocking step is implemented and verified.
- Existing telemetry hook detects *any* `/etc/hosts` mount as its own; Homebrew
  mounts that file first. The old hook updates an unmounted temp file instead.

## Upstream checkouts

Ignored Git clones under `tv/upstream/`:

- [PearWebOS](https://github.com/Arnedeklerk/PearWebOS),
  `7d4a66e9f652c075d371b490cececd3e90e7945e`.
- [lg-lockout](https://github.com/HunterAP23/lg-lockout),
  `e393103d89004617521c552675532ed6b7395f7a`.
- [webos-debloat](https://gist.github.com/lucataco/7c303b443934008a9031a55cfca552ec),
  `a269896606b83a31bb85f2f25146b51ceb876fe6`.

## Local compatibility checks

`devenv shell -- just tv-check` successfully applies Pear's required patches
to this TV's backed-up QML. Optional edits absent on this firmware were skipped
by the upstream validator. This validates patch compatibility, not rendering.

The local build adds Media Player and the invisible Alexa companion to Pear's
existing hidden-app list. It disables upstream's optional Live-TV guard and
power/update-setting writes for the first test. No signed firmware is modified.

The TV exhibits the upstream-documented `luna-send -n` empty-output issue when
captured without a TTY. State capture uses Pear's bounded `luna-send-n` helper;
the first empty JSON captures were retained separately as `raw-baseline`.

## Verification status

- [x] Branch `tv`, three local clones, private baseline and screenshot.
- [x] Required Pear patches match the actual firmware.
- [x] Pear rendered and navigable; stock rollback tested.
- [x] Alexa payload/launch paths blocked and tested.
- [x] Promotions/preloads/privacy changes checked.
- [x] Full reboot and standby/wake persistence checked.
- [ ] Step 9: OpenHome integration (explicitly deferred).

## Applied changes and evidence

- Pear uses three bind-mounted QML files. Stock Home was visually restored
  with `off`, mounts were confirmed absent, and Pear was enabled again.
- Sports Alert, Home Dashboard, Media Player and Alexa are absent from Pear's
  dock/drawer. They are not physically deleted from the signed firmware.
- Alexa apps `amazon.alexa.view` and `amazon.alexapr` are locked through SAM;
  direct launch returns `errorCode=-102`, `app is locked`.
- Empty-directory masks hide Alexa web payloads in the host and existing jail
  views. Non-executable file masks cover `amazon-alexa-adapter` and
  `amazon-alexa-vsk`. Their two LS2 service definitions use `Exec=/bin/false`.
- **A real bypass was caught:** the first native activation created a fresh
  jail whose overlay ignored the host's child-file mounts. Masking the current
  jail alone was insufficient. LG's supported `ls-control scan-services`
  refreshes cached activation commands without restarting the hub. After this,
  attempted activation leaves both native services disconnected and no native
  Alexa processes. Rollback/unmask/unlock/rescan was tested, then reapplied.
- A boot-only PATH issue was reproduced in the first reboot and fixed using
  `/usr/sbin/ls-control`; a minimal-PATH replay and a second reboot verified it.
  Boot stages retry independently so a privacy failure cannot skip Alexa.
- Hosts blocking now appears in the actual effective file and the real
  resolver returns `::1` for a representative telemetry host. Repeat application
  does not duplicate its section/mount. Restore removes only its owned mount,
  leaving Homebrew's OTA entries; restore/reapply was tested.
- Targeted settings read back correctly: home/live promotions, sports alerts,
  promotion videos, watched-list collection and recommendations off. Live Plus,
  debug/fault log uploads were already off and remain off. Unsupported browser
  cookie controls were not inserted.
- `/tmp/acr.xml` initially had capture/send enabled despite Live Plus being off.
  A longer standby test caught these returning to true after the boot rechecks
  had ended. The targeted config now uses a read-only bind of a sanitized copy,
  preventing wake-time rewriting; all `ACR_On`, `capture` and `send_data` true
  flags are false. A fresh wake cycle verified this fix without manually
  reapplying settings. Bounded boot checks also handle late creation of the file.
  No guessed ACR service was restarted.
- Alexa's original `isAllowed=false` preload exclusion is preserved. An initial
  runtime `isEnabled=false` change reset on standby wake; it was unnecessary
  while the app is disallowed, so that toggle was removed rather than adding
  a daemon to keep rewriting it. The live check asserts the actual persistent
  exclusion. No preload config file was rewritten; other choices are untouched.
- Ordinary app-update caches, consents, Content Store/voice/ThinQ domains,
  Quick Start+, power-on choices and firmware-update settings were preserved.
  Existing Homebrew OTA protection remains enabled.

## Installed paths

- `/var/lib/pearwebos`: generated Pear files and upstream lifecycle tools;
  `GUARD=0`, `BOOT_TO_HOME=0`, `QUICK_START=0`, `NO_UPDATE_PROMPT=0` prevent
  upstream optional daemons/settings writes. Existing Quick Start+ stays on.
- `/var/lib/openhome-tv`: task scripts, mask sources and original hook/XML copies.
- `/var/lib/webosbrew/init.d/10-block-lg-telemetry`: repaired targeted hosts hook.
- `/var/lib/webosbrew/init.d/40-openhome-tv`: bounded Alexa/privacy boot work.
- `/var/lib/webosbrew/init.d/60-pearwebos`: Pear boot mounting/cold start.
- Original Homebrew `startup.sh` and existing Input Hook shortcuts are unchanged.

## Final validation and limits

`devenv shell -- just tv-check` validates shell syntax and builds Pear from the
real backed-up firmware. `just tv-verify` runs the live launch/activation,
process/payload, settings, preload, XML and real-resolver checks. These checks
passed after reboot. See `ROLLBACK.md` for exact per-change restoration.

Three software full-reboot transitions were observed (SSH down/up and uptime
reset; this TV restores a quick-boot kernel image, so boot UUID alone does not
distinguish them). The final startup recreated the read-only ACR override,
reapplied Alexa/privacy changes, and Pear's running Home/checkpoint stamp
matched. Homebrew failsafe cleared normally.
Cold boot can show LG/no-signal content while startup hooks run before Pear;
ordinary Quick Start+ wake resumes the existing Pear process.

IR power-off/on cycles through the existing OpenHome CLI suspended SSH, then
returned the TV to `Active`, foreground LG Home running Pear, same Home PID and
three mounts. `screenshots/after.jpg` is a real capture of the final running
Pear Home, refreshed after the final reboot; private captures also record wake.

Navigation/drawer, YouTube app opening to its account picker, HDMI 1 selection,
Quick Settings and returning Home were checked on the real display. No account
was selected, no agreements accepted, and audible playback/casting were not
tested end to end. The physical Alexa button was not separately observed;
its target apps and both native activation paths were tested directly.

SSAP ports 3000/3001 accept loopback connections on the TV and
`network:allowMobileDeviceAccess=true` is preserved. Direct connections from
the workstation are refused despite those listeners; no network/firewall
configuration was changed to bypass that boundary. Existing OpenHome control
uses IR, and its Home command was verified on the display.

Hosts blocking is a selected hostname policy, not proof that every possible
tracking connection is prevented. Firmware bytes remain; root can undo masks.

## Step 9 — later work

Create our own Pear-based launcher with OpenHome integration after this working
baseline. Follow `CONTEXT.md`: user-facing controls go through the Axum API and
its existing integration boundaries. No integration UI or product behavior was
implemented in this task.
