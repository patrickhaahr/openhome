# LG TV work log

## Scope

Approved: install and verify Pear, hide Sports Alert, Home Dashboard, Media
Player and Alexa; prevent Alexa execution; selectively disable promotions,
ACR and unwanted preloads; repair telemetry blocking. Keep normal app updates,
HDMI, playback, SSH, casting and local OpenHome/SSAP control available.

Steps 1–8 were saved as the working Pear baseline in `5448646`. The user then
authorized step 9: a native OpenHome launcher and live household controls.

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
- [x] Step 9: native OpenHome integration and fullscreen agent visual concept.

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
three mounts. Private captures record Pear Home after the final reboot and wake.
The current public before/after launcher screenshots are shown in [README](README.md#screenshots).

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

## Step 9 — OpenHome launcher

Replaced Pear's wallpaper UI with tracked `launcher/OpenHome.qml` and small native
QtQuick components. Pure black, bundled Familjen Grotesk, large apps and clock,
today's weather, four room controls and Charge/Rest/Effort from Axum. Date, null
values and stale states follow the API contracts; no client-side Rest formula.

The TV-local Node8 client exposes only four actions plus sanitized weather and
recovery over loopback. The shared API key is root-owned mode600 on the TV and
never appears in QML/Git. HTTPS uses Pi Tailscale100.120.132.42 with verified
Host/SNI `openhome.haahr.me`. The existing router `tv_to_ts` rule already allows
this path. Read-only router/Pi inspection established Pi LAN10.0.10.4 and zaza
10.0.10.3. No firewall, Pi, dashboard or Axum product changes were required.

The Node client has bounded requests, no command retries, a single command slot,
rate limiting and a supervised boot hook `45-openhome-client`. Review found stale
directory-lock and shutdown races; kernel `flock` and bounded Node shutdown replace
them. Three concurrent cold starts produced one supervisor and one client.

A full software reboot was verified by SSH down/up and uptime4412→85 seconds.
The new client and all three Home mounts returned automatically, with Home's
checkpoint stamp matching. Post-reboot `tv-client-check` and `tv-verify` passed,
including actual Alexa activation attempts, privacy settings, read-only ACR and
the existing resolver/local-control protections. The final TV was left on live Home.

Verified on the TV: authenticated fresh API reads; all three scores present;
loopback-only listener and origin/proxy rejection; remote focus/navigation, app
drawer, YouTube launch and return Home; light and speaker command acknowledgements.
Acknowledgements do not establish physical light power or speaker input. Audible
playback and a physical remote microphone stream remain unverified.

Ember is the new house-shaped mascot. It opens fullscreen and OK toggles animated
radial strokes/mouth for a speaking demo. No recording, speech model, assistant
network request or Hermes connection exists. The Magic Remote microphone is
documented by LG; access to its audio under root is separate future research.
The user confirmed the physical display stays complete during the speaking demo;
intermittently missing unchanged elements occur in software captures only.

### Owner's visual revision

Removed all visible prose, headings, labels, app captions and instructions. Kept
the clock, current temperature and scores; the only words are the Danish date.
Used the dashboard's actual Phosphor icons, including filled recovery symbols,
filled/outline bulbs and waveform for optical. Controls are vertical capsules;
app icons remain large. LG's Apps tile (`com.webos.app.discovery`) is hidden.

The face mascot was replaced with a cyan/blue circular interface matching the
owner's reference: segmented annuli, compass ticks, circuit lines and a reactive
core. Static vector layers are compiled to PNG; the speaking demo uses native
transform animations. No audio/model/Hermes functionality was added.

Verified remote focus on the agent and all four control positions on the TV.
The drawer initially fitted five columns despite six-column navigation; its
width now includes the left margin, and Down selects the icon directly below.
Native client checks passed against the authenticated live API after deployment.

### Speaker, artwork and blue-light follow-up

Added the dashboard’s SpeakerHifi header, removed the drawer X, and made
`org.webosbrew.inputhook` visible. LG's live model handles the owner's removed
apps; nothing was reinstalled. Opaque image edges are sampled locally and their
colour fills the rounded card; logos preserve their aspect ratio. Transparent
logos retain their previous size.

Read `picture.eyeComfortMode` through the TV's settings service. Its descriptor
reports a global, nonvolatile `on`/`off` setting, initially `on`. The native Home
service can read and write it. Tested the actual remote button from on to off,
verified via an independent Luna read, then toggled back and verified on.
The launcher reads before toggling and after writing, rather than predicting
state. Failures clear the displayed state. No room-control API or firewall
changes were needed.

### YouTube repository migration and stronger warmth

Added the owner's [RF1705 repository](https://raw.githubusercontent.com/RF1705/youtube-webos-cobalt-adfree/main/repo.json)
through Homebrew Channel's supported addRepository launch parameters and TV UI.
The URL is persisted in Homebrew's localStorage. Its custom entries appear at
**the top** of the catalogue, above the default repository's alphabetic list.
The default repository also has a different YouTube AdFree package under the
same `youtube.leanback.v4` ID; use the **RF1705 YouTube Cobalt AdFree** entry.

The original manually installed app was `com.cobalt.youtube.adfree` v1.1.7.
At the owner's request, installed stable `youtube.leanback.v4` v1.2.5 using
Homebrew's install service and the repository's SHA256 verification:
`189af7d46617a511b191dc472a9fcfec3d0f44caf70107fb1da9adde3d39f2de`.
Verified it launches into YouTube's welcome/setup screen, then removed the
legacy app through Homebrew's uninstall service and verified its ID is absent.
The old app directory is backed up in ignored `private/youtube-legacy-1.1.7.tar.gz`.
Sign-in and playback are not claimed by this launch test. No beta was installed.

The picture stored `colorTemperature: -50` (Warm 50) with
`eyeComfortMode: on`; that slider is inactive in Cinema mode. The current
default-input/SDR/Cinema profile instead used
`whiteBalanceColorTemperature: warm2`; its supported presets extend to warm3.
Changed that one setting to **warm3**, verified via Luna readback, and kept
Reduce Blue Light enabled. This changes the native TV picture, not a QML tint.
Other picture profiles and RGB calibration values were not changed. Software
captures do not measure the physical screen's emitted blue light. The prior
values are in ignored `private/picture-before-warm3.json`; targeted rollback is
in ROLLBACK.md.

Removed the speaker header and enlarged the eye control from 72 to 104px, with
its own matching 124px capsule and the same right-rail alignment and focus ring.
Tested the enlarged button off and back on; independent Luna reads confirmed
Warm 3 persists in both states. Homebrew’s RF1705 details page recognises the
installed v1.2.5 and shows its disabled Update button (already current).

### Separate warm/dim picture profile

The owner chose a profile switch instead of changing the current profile further.
Preserved the exact current Cinema settings, including the previously applied
red0/green−10/blue−30 gains and the owner's brightness65/contrast65/color60 values.
Saved them in ignored `private/picture-normal-preserved.json`. The pre-adjustment
Cinema gains remain in `private/picture-before-amber.json`.

Selected `expert2` (Expert Dark Room), backed up its original settings to ignored
`private/picture-expert2-before.json`, then copied Cinema's core picture controls
with Warm3, backlight30, green gain−15 and blue gain−40. Red and RGB offsets were
preserved. Readback verified every written value; selecting Cinema again restored
all queried normal values exactly. The `brightness` black-level control stayed65;
actual light output is reduced through `backlight`.

The native eye component now reads/writes `pictureMode` (`cinema`/`expert2`) and
reads back the result. It no longer changes `eyeComfortMode`, which stays as the
owner had it. A white eye identifies the warm/dim profile; a dark eye identifies
Cinema. These presets are configured for the current default-input SDR context.
The actual remote eye was tested in both directions: expert2/backlight30 on,
then a complete equality check against all preserved Cinema values on return.
The screenshot was refreshed after the owner moved YouTube into the first slot.

### Reference-oriented Cinema baseline

At the owner's request, replaced the preserved amber Cinema settings with the
[documented film baseline](CINEMA.md). Warm2/neutral RGB replaces Warm3 and the
previous arbitrary gain reductions; brightness50, colour50 and contrast80 match
the firmware's ordinary LCD Cinema defaults. Gamma2.4 and backlight50 are an
evening-viewing baseline, not a measured luminance or gamma calibration.
Disabled dynamic colour, noise reduction, local dimming and TruMotion; retained
Auto gamut and Real Cinema. Other already-disabled enhancement controls stayed
off. The unsupported legacy edge-enhancer key was not forced.

Global Reduce Blue Light and automatic energy saving are off to avoid shifts in
colour and output. The night profile's 78 readable profile-scoped controls were
unchanged, although those global switches also affect its displayed appearance.
All 26 submitted ordinary picture controls, both global switches and the
energy-saving manual-change flag were read back. Merely setting Energy Saving
Off reverted to Auto on a profile switch; setting the native
`energySavingModified: true` flag kept it Off through the tested round trip.
Explicit app-context queries verified TruMotion off for Home, YouTube and
Netflix. The actual remote eye switched to the preserved night profile and back
to Cinema; native equality checks passed in both directions. Private snapshots
and targeted rollback details are retained. No measured panel calibration or HDR
playback verification is claimed.

### Red-button picture shortcut

The owner selected the red remote button. Live Input Hook 1.5.1 configuration had
no binding for key 398; its log confirmed physical red events use 398. Added one
`exec` binding without changing the five existing number-key app shortcuts.
Backups of the exact original map are retained on the TV and in ignored
`private/inputhook-before-picture-shortcut.json`. Input Hook hot-reloaded the map;
its existing boot hook is unchanged. The owner confirmed the physical button
worked, and native readback observed the warm profile at backlight 30.

The helper reads the current native mode, writes only `pictureMode`, and reads
back the result. It rejects contexts outside the configured default-input 2D SDR
profiles. A nonblocking kernel file lock prevents overlapping shortcut commands.
Home's eye subscribes to native changes and cancels that subscription when hidden;
private display captures verified its white/night and dark/Cinema states without
the prior polling delay. Current public screenshots were preserved.

The first helper's per-key Node startup caused the owner's reported delay. A live
timing assertion requiring completion below 1 second reproduced the issue at
2.418 seconds. Node startup alone measured 1.991 seconds, versus 0.158 seconds for a
native settings read. Replaced the helper with a small script compatible with
the TV's existing Python 2.7.16 and local Python 3. The same full-command assertion
then passed in both directions at 0.837/0.836 seconds, including readback. Native
profile values, global controls and other Input Hook bindings were preserved.
Timing evidence is in ignored `private/picture-shortcut-timing.json`.
Also exercised the installed command in both directions with YouTube in the
foreground: the app retained focus and the starting profile was restored before
returning Home. The live OpenHome client acceptance check passed afterward.

### YouTube shortcuts through Input Hook

Updated Input Hook key 2 (remote 1) from the removed `com.cobalt.youtube.adfree` app
to the installed `youtube.leanback.v4` v1.2.5. Added key 1037 (Netflix) with the same
native `launch` action. Netflix previously had no Input Hook override. Confirmed
the installed app's actual ID before writing, saved the exact prior map locally
and on the TV, and verified both entries after Input Hook's file reload. Every
other entry, including the red picture-profile shortcut, matched the prior map.
No extra app, daemon, command helper or boot hook was needed.
The live Input Hook log recorded both physical keys invoking YouTube, and the
owner confirmed that both 1 and Netflix opened it successfully.

See [design and public data provenance](DESIGN.md), [API boundary](bridge/README.md)
and [rollback](ROLLBACK.md). Original Pear remains under `/var/lib/pearwebos-baseline`.
