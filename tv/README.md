# OpenHome TV

Working branch: `tv`. Device: LG 55NANO806PA, webOS 6.5.3 / firmware 03.53.45.

- [Findings, changes and verification boundaries](FINDINGS.md)
- [Exact rollback commands](ROLLBACK.md)
- [Primary-source research](RESEARCH.md)
- [Before LG Home](screenshots/before.jpg)
- [After Pear, captured after the final reboot](screenshots/after.jpg)
- [Pear app drawer](screenshots/after-apps.jpg)
- [OpenHome launcher design](DESIGN.md)
- [OpenHome TV Home, synthetic public data](screenshots/openhome.jpg)
- [Ember speaking concept](screenshots/ember-speaking.jpg)

## Layout

`upstream/` contains the three Git clones at the revisions in `FINDINGS.md`.
`private/` holds proprietary firmware copies, personal settings, screenshots,
SSH socket/known-hosts and the 69 MiB baseline backup. Both directories are
ignored by Git; do not publish their contents. No SSH password is stored.

Qt tooling parses the QML locally; the TV itself validates its QtQuick 2.12 runtime.
Run project commands from the OpenHome root:

```sh
devenv shell -- just tv-connect
devenv shell -- just tv-check
devenv shell -- just tv-verify
devenv shell -- just tv-screen home
devenv shell -- just tv-client-check
devenv shell -- just tv-deploy
```

`tv-check` needs the ignored upstream checkout and private baseline; it has no
TV side effects. `tv-verify` attempts the blocked Alexa launches/activations and
reads their outcomes plus privacy settings. It needs the TV on and SSH connected.
`tv-screen` captures the actual display into `private/<name>.jpg`.

Initial installation recipes intentionally refuse to overwrite an existing installation.
`tv-deploy` updates the OpenHome UI/client and preserves the original Pear UI on the TV.
Root changes live under
`/var/lib` and runtime bind mounts, never in signed firmware partitions.

Step 9 now includes live weather, Charge/Rest/Effort, lights on/off and speaker
Bluetooth/optical commands. Ember is a fullscreen visual concept with a manual
speaking animation. Microphone capture, speech models and Hermes are not implemented.

The client runs independently of Home under `/var/lib/openhome-tv`, listening only on
`127.0.0.1:39479`. The API key is piped from the existing CLI configuration to a
root-owned mode600 file, never included in QML or Git. See [the client contract](bridge/README.md).
Home calls only the local allowlist; the client calls Axum using verified HTTPS.
The existing TV→Pi Tailscale firewall rule already permits this route, so no network
configuration changed. Live verification found Pi LAN10.0.10.4; 10.0.10.3 is zaza.

## Remote

Left/Right chooses an app; OK opens it. Up reaches the room controls; Right from
Optical reaches Ember. Down from apps opens the full app drawer. Back closes the
drawer or Ember; hold OK on an app opens LG's existing reorder view. Magic Remote
hover/click also works. No control selection is an observed device-state indicator.

## Future voice input

LG's [MR21GA diagram](https://www.lg.com/us/support/pdf/remotes/2021-Magic-Remote-MR21GA.pdf)
labels a microphone on the Magic Remote, which the [exact TV model](https://www.lg.com/uk/tvs-soundbars/smart-tvs/55nano806pa/)
supports. A separate far-field microphone in the TV chassis has not been confirmed.
LG developer staff [do not expose a microphone stream to ordinary webOS TV apps](https://forum.webostv.developer.lge.com/t/accessing-audio-data-stream/9355).
Root inspection found native microphone services, but no audio was captured or access
proven. Voice/Hermes design remains future work.
