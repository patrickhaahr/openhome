# TV debloating and privacy research

Date: 2026-10-21. Target supplied by the main investigation: LG 55NANO806PA,
webOS 6.5.3, firmware 03.53.45. This document records source review, **not TV
verification**. This research agent has not accessed or changed the TV.

## Sources inspected

- `upstream/lg-lockout`: commit `e393103d89004617521c552675532ed6b7395f7a`.
- `upstream/webos-debloat`: commit `a269896606b83a31bb85f2f25146b51ceb876fe6`.
- webOS Homebrew filesystem/settings documentation, Linux mount documentation,
  current webOS OSE SAM source, and source from independent rooted-TV projects.
  OSE behavior and other models do **not** establish retail webOS 6 behavior.

## Alexa: disabling execution is possible in principle, but paths must be verified

webOS Homebrew documents signed read-only SquashFS partitions and runtime bind
mounts for replacing files/directories. This supports reversible overrides; it
does not erase firmware bytes or reclaim their storage.
[Filesystem overlays](https://www.webosbrew.org/pages/filesystem-overlays).

The reported app IDs are `amazon.alexa.view` and `amazon.alexapr`. First inventory
each ID's `folderPath`, `appinfo.json`, `main`, executable/library references,
preload policy, active PID and services. Start with their reported
`/usr/palm/applications/<id>` directories, but also check alternate application
roots and writable updates. The reviewed debloater lists `/media/system/apps`,
`/usr`, `/mnt/otncabi`, `/mnt/otycabi`, and `/media/cryptofs/apps` app roots.
[Debloater application-root handling](https://gist.github.com/lucataco/7c303b443934008a9031a55cfca552ec).

**Do not equate hiding or preload denial with execution blocking.** The debloater
explicitly says blocked-system-app-list entries still launch when called.
Its settings and preload controls therefore cannot alone meet the Alexa goal.
[Debloater explanation](https://gist.github.com/lucataco/7c303b443934008a9031a55cfca552ec).

Recommended incremental implementation, subject to TV inspection:

1. Back up metadata and existing startup hooks. Identify and stop the two apps;
   target exact app IDs/PIDs, not a broad `amazon` process match (Prime Video is
   unrelated).
2. Keep preload denied for those IDs. Temporarily bind an empty directory over
   each **confirmed** app payload directory, then attempt explicit launches.
   Prefer this narrow payload block over changing core LG services.
3. Inspect whether a separate Alexa native adapter, unit or LS2 activation path
   exists. One independent project's source names `amazon-alexa-adapter`, but
   that is **only a search candidate**, not evidence that this model has it.
   Inspect `/usr/palm/services`, LS2 service definitions and actual init-system
   units for its real executable before selectively stopping/blocking it.
4. Inspect existing WAM/native-container processes and mount views. Test the
   Alexa remote button, explicit app launches, standby/resume and full reboot.
   Only call it disabled after these fail to start Alexa and no related process
   returns. Persist the narrow overrides in one dot-free Homebrew boot hook.
5. Undo by disabling that hook and unmounting only its recorded targets; reboot
   restores runtime mounts once the hook is disabled. Keep original metadata and
   preload values for full restoration.

The adapter search candidate comes from this project's own implementation; its
aggressive periodic kill loop is **not** recommended for this task.
[looking-glass Alexa process list](https://github.com/richknowles/looking-glass/blob/33d9abff1f651acd9cdf6198ffb5836650de97d8/lg-spy-killer.sh#L28-L42).

### Why a bind mount alone is insufficient proof

- SAM can hold app/launch-point metadata in memory. OSE SAM's launch path first
  checks its running-app list and can relaunch an existing process; full app
  scanning populates an in-memory description map. Therefore masking files does
  not prove that an existing app stopped. **Retail behavior is unverified.**
  [SAM launch/relaunch](https://github.com/webosose/sam/blob/96161d3e50670fb21daccfb12a75c35fe406ce0e/src/bus/service/ApplicationManager.cpp#L242-L276),
  [SAM scans](https://github.com/webosose/sam/blob/96161d3e50670fb21daccfb12a75c35fe406ce0e/src/base/AppDescriptionList.cpp#L118-L195).
- Mount namespaces can have different views; later mounts do not normally
  propagate to private existing namespaces. Inspect `/proc/<pid>/mountinfo`
  and `/proc/<pid>/root/...`, not merely the SSH shell view. App/jail mount
  propagation on this TV is **unverified**. A fresh post-hook launch/reboot is
  the strongest practical check.
  [Linux mount namespaces](https://www.man7.org/linux/man-pages/man7/mount_namespaces.7.html).
- A separate native Alexa service outside the app directories is unaffected by
  those directory masks. Conversely, a `noexec` flag alone does not block a web
  application read by an interpreter. The Linux flag concerns direct program
  execution; an empty payload mask actually removes accessible app content.
  [Linux mount flags](https://www.man7.org/linux/man-pages/man2/mount.2.html).

OSE also has `lockApp` and rejects launches of a locked app, but locks initialize
false and availability/semantics/persistence on retail webOS 6 are unknown. This
is a potential additional layer only if the TV exposes and demonstrably enforces
it; it is not a replacement for service/process verification.
[lockApp handler](https://github.com/webosose/sam/blob/96161d3e50670fb21daccfb12a75c35fe406ce0e/src/bus/service/ApplicationManager.cpp#L385-L408),
[launch policy](https://github.com/webosose/sam/blob/96161d3e50670fb21daccfb12a75c35fe406ce0e/src/manager/PolicyManager.cpp#L32-L46),
[initial lock state](https://github.com/webosose/sam/blob/96161d3e50670fb21daccfb12a75c35fe406ce0e/src/base/AppDescription.cpp#L175-L182).

## Privacy: adapt individual mechanisms; do not install all lg-lockout defaults

`10-block-lg-telemetry` at the reviewed commit checks its own marker in the
effective hosts file instead of merely checking whether `/etc/hosts` is mounted.
It builds from the current effective file, preserving preceding Homebrew entries,
and stacks its own bind mount. This directly addresses the **reported** installed
hook bug where Homebrew's existing mount triggered a refresh of an unmounted temp
file. Compare the actual installed script and verify the marker/domain entries
through relevant process roots after applying; a mount listing alone proves
nothing about the intended entries.
[Current hosts implementation](https://github.com/HunterAP23/lg-lockout/blob/e393103d89004617521c552675532ed6b7395f7a/10-block-lg-telemetry#L109-L151),
[upstream bug history](https://github.com/HunterAP23/lg-lockout/blob/e393103d89004617521c552675532ed6b7395f7a/CHANGELOG.md#L9-L12).

The host list includes OTA and ThinQ/IoT services, not just advertisements.
Content Store hosts `lgtvsdp.com` and `lgappstv.com` are commented out, with a
warning that blocking them breaks Content Store. Preserve them, preserve any
existing OTA/root-protection choices, and avoid adding broad LG wildcard blocks.
Hosts entries match listed names; they are not domain-family wildcard rules.
Existing DNS protection is outside the scope of these local scripts.
[Host categories and optional Store blocks](https://github.com/HunterAP23/lg-lockout/blob/e393103d89004617521c552675532ed6b7395f7a/10-block-lg-telemetry#L35-L98).

`20-decline-lg-tos` clears all discovered `*Allowed` flags and every agreement's
accepted state, includes `networkAllowed` and `generalTermsAllowed` in its
fallback, and restarts `eula-service`. It also disables LG voice and other
features. These are broader changes than targeted ACR/privacy settings; preserve
general agreement/account state and unrelated voice functions. Do not use its
file-edit fallback that can insert unsupported keys. Query and back up supported
settings, apply typed values through settingsservice, verify successful replies
and readback, and fail visibly for unsupported keys.
[Consent and setting implementation](https://github.com/HunterAP23/lg-lockout/blob/e393103d89004617521c552675532ed6b7395f7a/20-decline-lg-tos#L48-L169).

`30-clear-lg-app-update` moves aside and empties app-install service update caches.
Skip this hook to retain normal app-update behavior. Upstream `install.sh`
installs every hook and enables Homebrew's OTA-block flag, so do not run its
blanket installer.
[Cache-clearing hook](https://github.com/HunterAP23/lg-lockout/blob/e393103d89004617521c552675532ed6b7395f7a/30-clear-lg-app-update#L23-L58),
[Installer mutations](https://github.com/HunterAP23/lg-lockout/blob/e393103d89004617521c552675532ed6b7395f7a/install.sh#L60-L75).

### Narrow candidate settings and ACR handling

The debloater uses `general:homePromotion`, `general:sportsAlarm`,
`option:livePromotion`, `option:webOSPromotionVideo`, `option:promotionStreamer`,
`option:livePlus`, and `other:contentRecommendation`, all `"off"`. It is tested on
webOS 26 only. Confirm categories, existence and value types against this TV's
settings descriptions/current values before applying.
[Debloater settings list and tested-version declaration](https://gist.github.com/lucataco/7c303b443934008a9031a55cfca552ec).

Additional privacy candidates from lg-lockout are `option:watchedListCollection`
(`"off"`), `dbgLogUpload`/`faultLogUpload` (`false`) and `thirdPartyCookie`
(`"off"`). Treat their actual availability and consequences as unverified until
read and tested on this firmware. Query/write the owning settings service rather
than editing its backing files.
[Setting values](https://github.com/HunterAP23/lg-lockout/blob/e393103d89004617521c552675532ed6b7395f7a/20-decline-lg-tos#L48-L54),
[Homebrew settingsservice guidance](https://www.webosbrew.org/pages/hidden-system-settings).

For ACR, upstream code changes `ACR_On="true"` and `capture="true"` in
`/tmp/acr.xml`, writes a legacy opt-out marker, and optionally restarts a named
service. **Its README also claims to change `send_data`, but current code does
not do that.** Its early-return test ignores `send_data` too. Do not copy that
assumption: inspect this TV's XML/schema and actual owner first, back it up,
verify all relevant attributes, and avoid restarting a guessed unit. XML changes
and the opt-out marker remain model-dependent; neither by itself proves that
all tracking stopped. Upstream schedules delayed reassertions because services
can overwrite runtime state.
[Actual ACR implementation](https://github.com/HunterAP23/lg-lockout/blob/e393103d89004617521c552675532ed6b7395f7a/20-decline-lg-tos#L171-L190),
[reassertion scheduling](https://github.com/HunterAP23/lg-lockout/blob/e393103d89004617521c552675532ed6b7395f7a/20-decline-lg-tos#L197-L211).

Preserve `network:allowMobileDeviceAccess`: Homebrew documents it as the switch
for local SSAP/LG Connect Apps on ports 3000/3001, directly relevant to later
OpenHome control. Do not confuse it with consent's `networkAllowed` field.
[SSAP setting](https://www.webosbrew.org/pages/hidden-system-settings).

## Required evidence before declaring success

- Both Alexa IDs fail fresh direct launches; no Alexa processes/services return
  after remote-button use, reboot, and standby/wake. Firmware bytes remain.
- Correct hosts entries visible in the effective process views, not only a temp
  file; representative lookups return blocked addresses. This checks name-based
  blocking, not every possible telemetry connection.
- Supported ACR/promotion/sports settings retain requested values after reboot;
  XML/owner checks only where relevant files/services actually exist.
- Store and ordinary streaming apps, HDMI, audio, Settings, Homebrew, SSH and
  local SSAP remain functional; rollback procedure is tested independently.
- Until those live checks exist, label implementation as applied/unverified,
  never fully disabled or telemetry-free.

## Tailored hosts implementation

`privacy-hosts.sh` is a local adaptation of pinned lg-lockout's hosts script,
with its full MIT copyright/permission/warranty notice preserved. It adds only
the upstream telemetry/advertising/AI names; it omits new OTA, ThinQ/IoT,
Content Store and voice blocks (including `netflixvoice.lgtvcommon.com`). Existing
base hosts lines, including any existing OTA entries, remain intact. Domain
classification comes from upstream, not traffic analysis on this TV.

Default invocation applies; `--restore` removes one verified owned bind only.
The marker is `# OpenHome LG privacy block BEGIN`. Ownership is checked using the
effective target's Linux `fdinfo` mount ID and device/inode, not the existence of
any hosts mount. Refresh updates the same backing inode; if that pathname was
deleted/replaced, a recorded owned mount is first removed and rebuilt over the
original underlying hosts. A foreign overmount or ownership ambiguity fails
without unmounting it. Existing lines outside this script's delimited section
are preserved, including lines appended after it. A mkdir lock prevents
overlapping runs; a stale lock requires inspection before removal.

Only shell syntax has been checked locally by this research agent. Parent live
tests must establish kernel/BusyBox support, effective entries, repeated-apply
idempotence and restoration. Existing base mappings, resolver caches, custom DNS
clients or namespace views can limit hosts blocking; it is not a network-wide
firewall or a proof that every telemetry destination is blocked.
