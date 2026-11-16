# LG TV work log: shortcuts and Input Hook

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

### Prime Video, Disney+ and Rakuten TV shortcuts

Added native Input Hook `launch` bindings for Prime Video (1038) → Litefin
(`org.litefin.app`), Disney+ (1042) → Crunchyroll (`com.crunchyroll.webapp`) and
Rakuten TV (1044) → Web Browser (`com.webos.app.browser`). Confirmed all three app
IDs against the live installed-app list. All three keys were previously unbound.
Backed up the complete prior map locally and on the TV, hot-reloaded Input Hook,
and verified that only these three entries changed. YouTube, red and the other
number-key shortcuts were preserved.
The live log recorded each physical button invoking its assigned app, and the
owner confirmed that all three opened the correct app.

### Number keys for lights and speaker inputs

Mapped the previously unused remote keys 6/7/8/9 (Input Hook 7/8/9/10) to lights
on/off and speaker Bluetooth/optical respectively. Each `exec` action runs native
curl against the existing loopback TV client's matching allowlisted command,
with the required `X-OpenHome-TV: 1` header and a 15-second deadline. The persistent
client handles API authentication and verified HTTPS; no credential enters the
Input Hook map, and no additional daemon or per-key Node startup is needed.

Confirmed the API through `openhome health` and the TV through
`just tv-client-check`. Saved the exact prior map locally and on the TV in
`inputhook-before-room-shortcuts.json`, then verified only these four entries
changed. Existing app shortcuts and the red picture toggle were preserved.
The owner tested the physical remote and confirmed all four device actions work.

See [design and public data provenance](DESIGN.md), [API boundary](../bridge/README.md)
and [rollback](ROLLBACK.md). Original Pear remains under `/var/lib/pearwebos-baseline`.

### Input Hook cold-start recovery (2026-10-22)

The owner reported that every remote shortcut stopped working after the next
day's boot. All 14 saved bindings were intact, but neither `lginput2` nor
`micomservice` had `libinputhookpp.so` loaded. Input Hook repeatedly launched
under an unprivileged UID in its jail, despite its elevated service definition
remaining on disk; the original unbounded startup call was stuck waiting.

A normal LS2 `scan-services` did not fix activation. Refreshing the volatile
developer-service manifests with `scan-volatile-dirs` restored root activation.
`50-openhome-inputhook` now performs that refresh before the upstream startup
hook. Its Python 2.7 helper uses Pear's native `luna-send-n` for reliable
noninteractive replies and bounds each child command to ten seconds. It neither
rewrites key mappings nor changes Homebrew's shared service launcher.

The regression fails against the original startup hook and passes with the
repair; failed registry refresh and unresponsive activation are also covered.
After a real TV reboot, Input Hook started as root automatically, both input
processes had the library loaded, Homebrew startup completed, and the saved
bindings remained byte-for-byte unchanged. The owner confirmed that physical
1/YouTube and red/picture-profile shortcuts work both before and after reboot.
Private pre-change files and evidence are under `private/inputhook-startup/`.
