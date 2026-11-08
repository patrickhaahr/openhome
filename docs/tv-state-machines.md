# TV lifecycle dispositions

Consolidated record for [#260](https://github.com/patrickhaahr/openhome/issues/260),
TV epic [#254](https://github.com/patrickhaahr/openhome/issues/254) and umbrella
[#240](https://github.com/patrickhaahr/openhome/issues/240). It maps each TV
inventory candidate to a disposition, owner, phases and exits. It records no gate
results: those belong to the issues and to the commands that produced them.

JavaScript and QML have no linear types. Where a lifecycle changed, mutually
exclusive phases are guarded variants built by one constructor and left through one
transition function; a stale callback is ignored because it no longer matches the
current variant's identity (an object or Luna token) or phase. Nothing here claims
compile-time consumption of old states, and no generic state-machine framework was added.

Dispositions: **refactored**, **already explicit**, **retained simple**, **deferred**.

## Inventory

| Candidate | Slice | Disposition | Owner and phases |
| --- | --- | --- | --- |
| Launcher OK-key hold | [#255](https://github.com/patrickhaahr/openhome/issues/255) | Refactored | `OpenHome.qml` `holdEvent` is the only transition owner of `holdPhase`: `idle`, `pressed`, `held` (hold elapsed while editing was ineligible), `releasing` (carries `releasingFrom`). Replaces the `holding` flag plus two independently stopped timers. Drawer, region and agent state stay independent policy, read only when the 700 ms hold elapses. Timing (700 ms hold, 150 ms release debounce) and key-repeat handling are unchanged |
| Launcher navigation (dock, drawer, controls, agent, Back) | #255 | Retained simple | `navigate`/`activate` are pure branches over `drawer`, `region`, `agentOpen` and indices, with no asynchronous exit. Merging them into a state enum would conflate independent policy |
| Launcher state read | [#256](https://github.com/patrickhaahr/openhome/issues/256) | Refactored | `readFlight` is null or `{request}`; `settleRead` is its only exit (valid data, invalid/offline marked stale, or 10 s timeout then abort). Late callbacks do not match the current flight and are ignored |
| Launcher command feedback | #256 | Refactored | `commandFlight` (`{request, command}`) is separate from `commandOutcome` (`{command, accepted}`); `pending`, `acknowledged` and `failed` are derived, so a command identity exists only while in flight and an outcome only after it. `settleCommand` is the only exit (accepted, uncertain, or 14 s timeout then abort); no retry. The 4.5 s timer clears only an accepted marker; an uncertain marker stays until the next send, as before |
| Picture profile read/toggle/write/readback | [#257](https://github.com/patrickhaahr/openhome/issues/257) | Refactored, with a narrow fix | `PictureProfile.qml` `operation` is null or `{kind, token, target}` with kinds `read`, `toggle`, `write`, `verify`; `begin` constructs it and `advance`/`finish`/`fail`/`mismatch` leave it. `pending` is derived. The observed `value` and subscription stay separate from the operation. **Fix:** readback now must equal the requested target; before, any readable mode counted as success. A mismatch fails the toggle but keeps the observed value |
| Picture shortcut script | #257 | Already explicit | `picture-toggle.py` is one sequential read, write, read-back and already fails with exit 1 if the readback differs from the target; unchanged. It also validates the default-input 2D SDR dimension, which the launcher's read does not (observed, not changed) |
| Bridge refresh and cache | [#258](https://github.com/patrickhaahr/openhome/issues/258) | Refactored (extraction only) | `createBridge`: `refreshing` is the one shared in-flight promise, cleared in `finishRefresh`; weather and recovery tasks are separate functions bound to their start day, with independent freshness deadlines. Behavior unchanged |
| Bridge command admission | #258 | Refactored (extraction only) | `admit` separates existence (404), in-flight slot (409) and the one-second rate deadline (429) from transport; the slot is released on success, rejection or invalid body, the deadline is never released |
| Supervisor lock, pidfile, child, disable marker | [#259](https://github.com/patrickhaahr/openhome/issues/259) | Retained simple | `client-service.sh` unchanged; see below |
| Input Hook startup and boot steps | #259 | Retained simple | Sequential, bounded and already explicit; see below |

## Non-terminal exits

"Demonstrated" means an automated test in this repository exercises it. Everything
else is stated from reading the source and is **assumed** until the physical check.

| Lifecycle | Exit | Evidence |
| --- | --- | --- |
| Hold `pressed`/`held`/`releasing` | Release then 150 ms activation; eligible 700 ms hold then edit; key repeat; Home hidden (`holdEvent("cancel")`); stale `holdElapsed`/`releaseElapsed` ignored by phase | Assumed (source). No QML runtime seam exists |
| Hold while controls or agent selected | Waits in `held` for release, then activates | Assumed |
| Hold with lost release while Home stays visible | Stays `pressed`/`held`; a liveness hypothesis, not demonstrated. Left unchanged and without a new timeout | Assumed |
| Read in flight | Valid data; non-200/malformed marks stale; 10 s abort; late completion ignored | Assumed (QML). The bridge half (valid, stale, invalid bodies) is demonstrated in `tv/bridge/test.js` |
| Command in flight | Accepted; non-accepted or malformed uncertain; 14 s abort; late completion ignored; feedback expiry | Assumed (QML). The bridge half is demonstrated in `tv/bridge/test.js` (accepted, 502, busy, rate, no retry) |
| Picture operation | Matching reply advances; malformed, rejected, mismatched readback or 5 s timer fails; deactivation cancels only the subscription; 60 s refresh resubscribes; restart clears it | Assumed (source). No test seam without a production hook |
| Bridge refresh | All bounded tasks settle; rejection or invalid body keeps same-day data stale; midnight drops stale-day results; restart clears memory | Demonstrated: `cache()` in `tv/bridge/test.js`, including invalid bodies and slot release |
| Bridge command slot | Released on success, rejection and invalid body; 1 s deadline survives failure | Demonstrated: `commands()` in `tv/bridge/test.js` (success, rejection and invalid body each followed by an admitted command) |
| Supervisor | Start; duplicate start; child exit then restart after 5 s; TERM/INT stops the child and removes the pidfile; the kernel lock is released on exit; the disable marker is checked between iterations; stale pidfile; log-trim failure under `set -e` | Assumed; see below. The log-trim exit is an unreproduced liveness hypothesis |
| Input Hook startup | Success; registry failure; unacknowledged reply; 10 s subprocess timeout | Demonstrated: `tv/tools/test-inputhook-startup.py` |
| Boot steps | Three attempts each; a failure in one step does not stop the other; a hang in either is bounded only by the outer 90 s `timeout -t 90` in the `install-hooks.py` boot hook | Assumed (source) |
| Picture shortcut script | Success or exit 1 on rejection, malformed reply or readback mismatch. A hung `luna-send-n` has no deadline, so the run could hold the caller's `flock -n` and block later presses | Assumed (source); the hang is an unproved non-terminal state, deferred (see below) |
| Bridge refresh liveness | An exception thrown inside a success handler would reject `Promise.all`, skip `finishRefresh` and leave `refreshing` set, so `/state` would return 503 until restart | Unreachable today: `weatherFrom` and `recoveryFrom` are total over JSON-derived input. Assessed from source, not reproduced; no hardening added |

### Supervisor ownership (#259)

The kernel `flock` on `client.lock` is the single-instance authority, and releases on
crash or power loss. The pidfile is an advisory pointer written only after the lock is
held, and `matches` accepts a pid only if its `/proc` command line is the supervise
invocation, so a stale or reused pid is not trusted. A duplicate `start` is safe: the
second supervisor loses `flock -n` and exits before touching the pidfile. The child
closes descriptor 9. TERM/INT kills and reaps the child (or the pause sleep), removes
the pidfile and exits. `stop` signals the pid and polls for five seconds. The
disable marker is checked only between child runs; it does not stop a running child.
The supervise loop runs under `set -eu`, and the log trim (`tail ... > client.log.tmp`,
`cat ... > client.log`) is unguarded. A failure there, such as a full disk, would exit the
supervisor silently, releasing the lock and leaving a stale pidfile. This is read from source
and not reproduced. `start` runs only from the boot hook and from deploy, so nothing
restarts the supervisor in between. It is recorded as a liveness hypothesis and left
unchanged. No race or stale-pid defect was demonstrated, so the script is unchanged. A
process-level test would need the hard-coded `/var/lib/openhome-tv`, `/usr/bin/node`
and `flock` device paths replaced by a test-only seam, so none was added.

### Retained sequential flows

`inputhook-start.py` (two subprocesses, each with a ten-second deadline, Python 2.7
compatible) and `boot.sh` (three attempts each for the Alexa and privacy steps) are
already linear with explicit failure exits; a state model would only restate them.
`boot.sh` runs the two steps independently for failure: one failing does not skip the
other. This does not hold for a hang. `alexa-block.sh` and `privacy-settings.sh` call
`luna-send-n` without a deadline (only `ls-control scan-services` has `timeout -t 15`),
so a hung call stalls the boot sequence until the 90 s `timeout -t 90` wrapper in the
`install-hooks.py` boot hook ends it. `boot.sh` also has its own `$DIR/disabled` marker,
checked once at start, separate from the supervisor's `client-disabled`.

### Picture profile dispositions

- **Default-input 2D SDR check: deferred.** `picture-toggle.py` rejects anything other
  than default-input 2D SDR; the launcher's read does not. Aligning the launcher would
  change launcher behavior beyond #257, whose candidate was the unchecked verify
  readback (now fixed).
- **Shortcut script deadline: deferred.** `picture-toggle.py` uses `subprocess.check_output`
  with no deadline, whereas `inputhook-start.py` bounds the same `luna-send-n` at 10 s.
  The shortcut runs under `flock -n /tmp/openhome-picture-toggle.lock`, so a hung run
  would hold that lock and block later presses. This is an unproved non-terminal state.
  Python 2.7's `check_output` has no `timeout=`, so a fix needs a poll loop like
  `inputhook-start.py`.

## Cross-boundary semantics

An accepted command (`{"state":"accepted"}`) acknowledges only that the Axum API
accepted the request; the launcher marks it "accepted" and never derives lights or
speaker state from it, and an unanswered command is shown as possibly sent. The
picture control is the opposite case: its success now requires an observed mode
read back from the TV equal to the requested one, and the written request alone
never establishes it.

## Evidence limits

- **No QML runtime coverage.** `qmlformat` in `tv-check` only parses the launcher; the
  hold, read/command and picture lifecycles are unproven by automation and need the
  physical remote.
- **Pending physical-TV acceptance:** remote OK short press, 700 ms hold and release,
  key repeat, Home hide while held; picture toggle in both directions, a forced
  mismatch, and a rejection; supervisor stop and restart; reboot and shortcut behavior.
- Bridge tests use a synthetic transport and an ephemeral loopback server, not the
  live API. The Pear build in `tv-check` needs locally backed-up LG Home files, so it
  is available only on the host that holds them.
- No hardware acceptance is claimed by this document.
