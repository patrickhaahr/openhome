# LG TV / Pear baseline

Working branch: `tv`. Device: LG 55NANO806PA, webOS 6.5.3 / firmware 03.53.45.

- [Findings, changes and verification boundaries](FINDINGS.md)
- [Exact rollback commands](ROLLBACK.md)
- [Primary-source research](RESEARCH.md)
- [Before LG Home](screenshots/before.jpg)
- [After Pear, captured after the final reboot](screenshots/after.jpg)
- [Pear app drawer](screenshots/after-apps.jpg)

## Layout

`upstream/` contains the three Git clones at the revisions in `FINDINGS.md`.
`private/` holds proprietary firmware copies, personal settings, screenshots,
SSH socket/known-hosts and the 69 MiB baseline backup. Both directories are
ignored by Git; do not publish their contents. No SSH password is stored.

Pillow/numpy are provided by the repository's devenv shell for the upstream
wallpaper builder. Run project commands from the OpenHome root:

```sh
devenv shell -- just tv-connect
devenv shell -- just tv-check
devenv shell -- just tv-verify
devenv shell -- just tv-screen home
```

`tv-check` needs the ignored upstream checkout and private baseline; it has no
TV side effects. `tv-verify` attempts the blocked Alexa launches/activations and
reads their outcomes plus privacy settings. It needs the TV on and SSH connected.
`tv-screen` captures the actual display into `private/<name>.jpg`.

Installation recipes intentionally refuse to overwrite an existing installation.
They are setup tools, not blind re-deploy commands. Root changes live under
`/var/lib` and runtime bind mounts, never in signed firmware partitions.

Step 9, a modified Pear launcher with OpenHome integration, is deferred.
