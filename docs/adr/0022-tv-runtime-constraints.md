# TV runtime constraints

The rooted LG webOS 6 TV runs OpenHome's launcher, bridge and privacy
measures. Its platform imposes constraints that the code cannot show.

- **Runtime.** The TV ships Node 8.12, so `tv/bridge/client.js` stays
  dependency-free and ES5-compatible. The client runs under a small supervisor
  that is independent of LG Home's CRIU snapshots. The supervisor holds a
  `flock`, which the kernel releases after crashes and power loss. Children
  close that descriptor so they cannot keep a dead supervisor's lock alive. The
  log is trimmed to 30 lines to bound disk use. A redeploy force-exits after
  1.5 s, so an old keep-alive listener cannot keep holding the port.
- **Network.** API requests keep the certificate hostname for Host and SNI
  while the TCP connection goes to the Pi's private address.
- **Bridge contract.** Each refresh task is bound to the Copenhagen day it
  started in, and a result that lands after midnight is dropped. The bridge
  returns only the display contract, never upstream objects. A command
  acknowledgement is never treated as device state: an uncertain outcome
  means the command may have been sent.
- **LS2 services.** `luna-send` can exit without a reply when stdin is closed,
  so Pear's `luna-send-n` comes first on `PATH`. `ls-control scan-services`
  leaves cached developer-service launchers unchanged, so Input Hook startup
  uses `scan-volatile-dirs`. A service that answers `returnValue=false` fails
  boot rather than counting as started. Input Hook's map is written in place,
  so its own file watcher reloads it. Verification tries to activate blocked
  services rather than only checking their status.
- **Alexa blocking.** Reversible payload masks cover only the Alexa paths
  verified on this TV: LS2 activation and direct execution. The hub's cached
  Exec lines are reloaded without restarting the hub or apps. Only the two
  native Alexa binaries are killed, and Prime Video is unaffected. Cached web
  apps close before their payload is masked. A privacy failure never prevents
  Alexa blocking, and vice versa.
- **Privacy hosts and settings.** The hosts policy is adapted from
  HunterAP23/lg-lockout, narrowed to telemetry, ads and AI. It adds no OTA,
  ThinQ, voice or Content Store blocks. It is a delimited section on a bind
  mount, so entries before and after it survive, including Homebrew OTA lines.
  The mount is identified through `/proc/self/fdinfo` even when bind mounts
  are stacked. Interrupted runs recover ownership only for the exact known
  backing inode with a distinct mount ID. Backing files are written before
  mounting, so readers never see partial content. Only the supported settings
  are changed: there is no broad EULA withdrawal. Rechecks after boot are
  bounded, with no polling daemon.
- **Pear build.** Upstream kills every client's foreground subscription even
  with `GUARD=0`, so the build patches that out. The guard stays off, with no
  Live-TV takeover, and power and firmware settings are left as they were.
- **Launcher UI.** It absorbs pointer and wheel input so LG's hidden shelves
  cannot steal focus. It probes only image edges, so transparent logos keep
  their sizing. Bundled fonts use native rendering.
- **Tools.** Tools never terminate a Node client other than the exact fixture
  command. A stale SSH control socket left by a reboot is cleared before
  reconnecting. The CLI's API key is read without being printed, and is piped
  over SSH in memory. The first deploy backs up the original UI once.
- **IR codes.** LG TV commands are NEC 32-bit frames taken from
  IvanGlinkin/Default-IR-vendors-samples (LG/TV) and trodemaster/rc
  (`LG_C4_IR_Codes.txt`).
