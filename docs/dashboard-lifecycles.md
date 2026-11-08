# Dashboard lifecycle dispositions

Consolidated record for [#274](https://github.com/patrickhaahr/openhome/issues/274),
Dashboard epic [#265](https://github.com/patrickhaahr/openhome/issues/265) and umbrella
[#240](https://github.com/patrickhaahr/openhome/issues/240). It maps every Dashboard
inventory candidate to a disposition and links the document that owns the detail. Like
[api-state-machines.md](api-state-machines.md) it records no gate results: those belong to
the issues and to the commands that produced them.

The approach was one owner per lifecycle, with mutually exclusive variants only where they
carry data a phase requires, constructed in one place. TypeScript does not enforce linear
consumption, so each owner is one guarded transition function or reducer plus identity
checks (a run number, a symbol token, an `AbortSignal`, a generation) that make an obsolete
callback a no-op. No state-machine library or shared framework was added. Independent facts
(policy, transport, command, observation, view data) stay separate fields rather than being
enumerated as combinations.

Dispositions: **refactored**, **already explicit**, **retained simple** (a direct branch or
sequence kept, with a rationale), **deferred** (with a linked explanation).

A guard that ignores a stale callback is not a guarantee that a callback arrives. Every
exit below is one of: proven by a test, supported by a named source, or listed under
[Liveness limits](#liveness-limits) as not demonstrated.

## Inventory

| Candidate | Slice | Disposition | Owner and rationale |
| --- | --- | --- | --- |
| Backlight queue and debounce | [#267](https://github.com/patrickhaahr/openhome/issues/267) | Refactored | `BacklightControls` holds one `Delivery` (`idle`, `debouncing`, `sending` with at most one waiting item, `closed`). The Dashboard's pending request stays separate from the API's `command`, `recovery`, saved intention and observation. See [backlight.md](backlight.md#dashboard-delivery) |
| Bedtime dial settle (700 ms) | #267 | Retained simple | One timer in a ref, cleared on every new edit and on unmount; its commit goes through the same delivery owner as every other action. `dashboard/src/components/bedtime-dial.tsx` |
| Backlight page poll (15 s) | #267 | Retained simple | `routes/backlight.tsx` runs one visible-only `router.invalidate` interval, cleared with its effect. It has no in-flight guard; a hung invalidate would only delay the view, not hold a command |
| List save and re-read | [#268](https://github.com/patrickhaahr/openhome/issues/268) | Refactored | `useListSave` owns one `Operation` through one `advance` function: `idle`, `sending`, `refreshing`, `settled` with `rejected`, `saved` or `saved-unread`. An acknowledged write is never downgraded by a failed re-read. See [wishlists.md](wishlists.md#dashboard-save-outcomes) |
| Sheet completion after close | #268 | Refactored | `useSheetLifetime` makes a late completion call `done` only while its sheet is mounted; the save's durable effects still happen. Same document |
| Wish retry identity | #268 | Refactored | `intendAddition` decides reuse or a new request ID in one function beside the storage helpers. The pending record keeps its `sessionStorage` key and shape; storage availability stays independent. [wishlists.md](wishlists.md) |
| Important Date retry identity | #268 | Retained simple | `DateAdd` already compares fields (`sameFields`) with one `openhome.pending-date` record, in memory when storage is unavailable. A shared helper would merge two different matching rules and change retry identity; a remounted form deliberately does not refill. [important-dates.md](important-dates.md) |
| Watchlist retry identity | #268 | Retained simple | Its pending plan is stored per owner (`openhome.pending-watch.<owner>`) and matched by the chosen title; unchanged, only the completion guard was added |
| Wish link read, media search | #268 | Retained simple | Each uses a debounce timer cleared on unmount and a ticket so only the newest answer applies; the status is display-only and never blocks a save. See [Liveness limits](#liveness-limits) |
| Slideshow playback | [#269](https://github.com/patrickhaahr/openhome/issues/269) | Refactored | One effect run owns what it draws: `Stage` is written only through `show()`, guarded by the run's `AbortSignal`; `Loaded` is `loaded`, `failed` or `cancelled`; load keys never repeat. `dashboard/src/components/idle-screen.tsx` |
| Deck advancement | #269 | Already explicit | `slideSource` and `dealPages` in `lib/idle.ts` stay a bounded sequential protocol: pages are dealt once in a seeded order, one page is read at a time, and a remembered position carries on across spells. A state model would restate the iteration |
| Idle and wake | [#270](https://github.com/patrickhaahr/openhome/issues/270) | Refactored | `reduceRoom` is the only transition boundary for `awake`, `idle`, `waking`; timers and listeners report events and never read state. `idle` and `waking` carry the focus to restore. `dashboard/src/components/idle.tsx` |
| Idle blockers | #270 | Already explicit | Each `useIdleBlock` holds its own token and releases it alone, so several reasons are held independently of the phase. `idle-block.ts` was not changed |
| Photo page requests | [#271](https://github.com/patrickhaahr/openhome/issues/271) | Refactored | `PhotoWall` owns one `PageRequest` (`idle`, `loading` and `failed`, each carrying its cursor) behind a single-flight token; loaded items and the next cursor stay separate view data. See [ADR 0009](adr/0009-immich-selected-albums.md) |
| Weather polling | [#272](https://github.com/patrickhaahr/openhome/issues/272) | Refactored | `LiveWeather` holds one `Polling` (`scheduled`, `reading`, `hidden`, terminal `disposed`) per effect generation; forecast freshness expires independently. See [ADR 0006](adr/0006-weather-provider-and-cache.md) |
| Map resource attempts | [#273](https://github.com/patrickhaahr/openhome/issues/273) | Refactored | `startMapAttempt` owns import, load timer and map for one attempt, moving importing, starting, running, released only forward; every exit releases through it. See [ADR 0007](adr/0007-shared-map-origin-and-rendering.md) |
| Map status, perspective, building coverage, gestures | #273 | Already explicit | `MapView`, `Perspective` and `BuildingCoverage` are separate small unions written by one path each; merging them with the resource would couple independent policy. Rotation and pitch stay disabled through library options |
| Display clock and Copenhagen day | none | Retained simple | `useMinute` and `useCopenhagenDay` each keep one timer and one visibility listener, both removed on cleanup, and re-sync when the page becomes visible |
| Date board refresh, idle date rotation | none | Retained simple | Each is one interval cleared with its effect; the board's refresh is visibility-gated and swallows its own rejection |
| Theme-turn class removal | none | Retained simple | `daylight.tsx` removes a cosmetic class after 1.4 s without cancelling; a stale removal cannot affect behavior |

No candidate is deferred. The residual items are limits, not unfinished slices.

## Non-terminal exits

For each changed lifecycle: its non-terminal states and how each is left. "Proven" means a
test in `dashboard/tests` drives it; "source" means a named bound the code relies on;
"not demonstrated" points to [Liveness limits](#liveness-limits).

| Lifecycle | Non-terminal state | Success | Failure | Cancel, replace, unmount | Timeout | Restart |
| --- | --- | --- | --- | --- | --- | --- |
| Backlight | `debouncing` | 200 ms timer sends (proven) | none | an action replaces it and clears the timer; unmount clears it (proven) | none needed | page reload starts `idle` |
| Backlight | `sending` | response delivers what waits (proven) | a rejection becomes the "No answer" outcome and still drains the latest waiting work (proven) | unmount drops the response (proven) | server abort at 65 s (source); browser call not demonstrated | page reload |
| List save | `sending` | acknowledged, then `refreshing` (proven) | a rejected receipt or thrown call settles `rejected` (proven) | closing the sheet does not cancel; the lifetime guard skips `done` (proven) | server 5 s (source); browser call not demonstrated | the retry record survives reload |
| List save | `refreshing` | `saved` | a failed re-read is `saved-unread` and still resolves `true` (proven) | a late step cannot overwrite a settled save (proven) | not demonstrated | the retry record is already spent |
| Slideshow | image load (`load()`) | `loaded` | `failed`; a run of failures rests (proven) | the run's abort settles the load as `cancelled` at once through its own listener; a late image event cannot settle a newer load (proven) | no app timeout: the image event on the browser hop is left only by load, error or abort (abort proven, event not demonstrated) | waking mid-load keeps the unseen photo (proven) |
| Slideshow | `prepare()` and page read (`source.next()`) | the call resolves and the run continues | a rejection or unavailable result rests the ground (proven for an unreachable library) | abort does not settle the call: the browser call to the server function stays pending after abort and the run's `await` ends only when the call it is awaiting settles. After `prepare()` settles the run may await `source.next()` and may issue a page request (not when `prepare()` gave no source, nor for an on-this-day photo), and returns without drawing once it reaches the abort check or the cancellation-aware catch; a late failure cannot blank a replacement (proven). Cancellation does not prevent that request | upstream 7 s / 45 s on the Axum request (source); the browser hop has no timeout or abort, so the old run can stay pending indefinitely (not demonstrated to end) | a replacement run starts empty and does not wait for the old one |
| Slideshow | dwell and rest | timer | none | abort stops it; waking mid-dwell leaves the position unmoved (proven) | the timer is the bound | the next spell continues the deck |
| Idle and wake | `awake` countdown | timer enters `idle` (proven) | none | a block or input restarts it (proven) | the countdown | none |
| Idle and wake | `waking` | grace elapses or release arrives (proven) | a lost release wakes at `WAKE_LIMIT_MS` (proven) | a block returns the room awake with focus (proven); unmount releases input capture (proven) | `WAKE_GRACE_MS`, `WAKE_LIMIT_MS` (existing) | none |
| Photo pages | `loading` | page settles to `idle` or ends at a null cursor (proven) | `failed` keeps photos and retries the same cursor (proven) | a settlement after unmount is ignored (proven) | server 7 s / 45 s (source); browser call not demonstrated | none |
| Weather | `scheduled` | timer starts a read | none | hiding cancels the timer; replacement or disposal clears it (proven) | the schedule | showing refreshes |
| Weather | `reading` | result applies and schedules | an unavailable result retries after 60 s (proven) | a read finishing after replacement or disposal is discarded; none overlaps within a generation (proven) | server 7 s (source); browser call not demonstrated | showing again never starts a second read (proven) |
| Weather | `hidden` | showing refreshes (proven) | none | a read settling while hidden applies and schedules nothing (proven) | none | none |
| Map attempt | `importing`, `starting` | `running` | provider or graphics failure releases (proven) | retry, unmount, hidden tab and 3D to 2D restart release; a late import or event does nothing (proven) | the load timer releases and offers retry (proven, including raster) | retry is a new attempt |
| Map attempt | `running` | none | a 3D graphics failure releases the map and restarts it flat with a stated reason (source: `shared-map.tsx`; the restart release is covered by the graphics-failure test) | unmount removes the map and listeners (proven) | none | retry is a new attempt |

## Liveness limits

These are unproved exits, not guarantees. No Dashboard timeout or retry was added for any
of them, because no source requires one.

- **Browser call to a server function.** [#267](https://github.com/patrickhaahr/openhome/issues/267),
  [#268](https://github.com/patrickhaahr/openhome/issues/268),
  [#269](https://github.com/patrickhaahr/openhome/issues/269),
  [#271](https://github.com/patrickhaahr/openhome/issues/271) and
  [#272](https://github.com/patrickhaahr/openhome/issues/272) all rest on server-side
  bounds that stop the Axum request, but the browser's call to the Dashboard server
  function has no timeout or abort here. A call that never answers keeps its phase until
  the page is left: Backlight `sending` (and the `router.invalidate` the page awaits
  afterwards), a list save's `sending` or `refreshing`, a gallery `loading`, and a weather
  `reading`. The slideshow's image events have the same hop and are left only by the
  run's abort. Two consequences follow. A stuck Backlight `sending` also holds the idle
  slideshow off, because the page's pending state is an idle blocker (`backlight-controls.tsx`;
  not covered by a test that mounts the page inside an `IdleProvider`). A stuck weather read
  is left only by disposal or by the effect re-running when the forecast is replaced.
- **Slideshow preparation and page reads.** `prepare()` (the album and on-this-day reads)
  and `source.next()` are browser calls to Dashboard server functions that take no abort
  signal. Aborting a run does not settle them: the cancelled run stays pending until the
  call it is awaiting settles. The server bounds the Axum request (7 s per album, 45 s for
  all photos), but the browser hop has no timeout here, so a call that never answers leaves
  that old run pending for the page's life. A replacement run is independent of it. After
  `prepare()` settles, the run may await `source.next()` and may issue a page request (not
  when `prepare()` gave no source, nor when the next photo is an on-this-day one); it
  returns without drawing once it reaches the abort check or the cancellation-aware catch.
  Cancellation neither prevents that request nor settles the old run. This is unproved,
  not guaranteed away, and no timeout or retry policy was added or is invented here.
- **MapLibre `load` handler.** MapLibre 6 calls `load` listeners without catching, so an
  exception there would leave the vector view loading with no timer. A source audit found
  no reachable throw (invalid layers arrive as `error` events; the only throwing call, a
  duplicate source id, cannot occur on a fresh style), but no test reaches `load` without
  WebGL, so this is unproved rather than guaranteed away. The timeout stop stays before
  overlay setup, as [ADR 0007](adr/0007-shared-map-origin-and-rendering.md) explains.
- **Display-only reads.** A never-answering Wish link read or media search leaves its
  "Reading" or "Searching" status until the next typed query or until the sheet closes. It
  does not hold a save, a block or any other state.
- **Backlight poll.** A never-answering page poll delays the view only.

## Independent facts

Kept separate by design, so none is a combination case of another:

- Backlight: what the Dashboard has asked for (`Delivery`) versus the API's `command`,
  `recovery`, saved intention and device observation.
- Lists: the save `Operation` versus the retry record (`sessionStorage` or memory) versus
  the board's server state.
- Slideshow: the stage being drawn versus the remembered deck position versus the unseen
  page reads.
- Idle: the phase versus the blocker tokens versus the focus to restore.
- Photos: the page request versus the loaded items versus the next cursor.
- Weather: polling activity versus forecast freshness.
- Maps: attempt resource versus `MapView` status, `Perspective` and `BuildingCoverage`.

## Preserved contracts

HTTP and server-function schemas, retry request IDs and storage keys, debounce and dwell
timings, existing timeouts, privacy boundaries and credentials are unchanged. No timeout,
retry, provider call or dependency was added.

## Behavior proven by existing tests

A map to the suites, not a record of a run.

- **Backlight** (`dashboard/tests/backlight.test.tsx`): stale work replaced, a rejection
  draining the latest selection, an older response not replacing newer feedback, and
  unmount dropping waiting and in-flight work.
- **Lists** (`wishlists.test.tsx`, `important-dates.test.tsx`; shared helper
  `tests/support/lost-response.ts`): lost responses reusing the request ID, `saved-unread`,
  a late completion after the sheet closed, and a late save that must not mark a wish done
  after it was undone (a regression written to fail on the pre-fix flow).
- **Idle and slideshow** (`idle.test.tsx`): wake, held-touch grace, lost release, several
  blockers, a block while waking, unmount input release, and a cancelled run's late page
  failure not blanking its replacement (a regression written to fail on the pre-fix
  source).
- **Photos** (`photos.test.tsx`): retry of the same cursor, one read in flight, and an
  obsolete result after unmount.
- **Weather** (`weather-lifecycle.test.tsx`): expiry, retry, hide and show with one read
  in flight, replacement and disposal.
- **Maps** (`shared-map.test.tsx`): timeout against a late import, hidden or graphics
  retry, raster timeout, release and retry, and the Leaflet fallback.

Only #268 and #269 recorded a defect reproduced before the fix. #267 and #270 to #273
changed ownership and make no fixed-defect claim.

## Acceptance outside automated checks

The automated Dashboard checks use synthetic data and a simulated DOM. These need a person
with the real device, and none has been run:

- **iPad and Safari**: touch, held-touch wake, keyboard, orientation, reduced motion and
  older-device performance.
- **Real WebGL and tile providers**: MapLibre rendering, graphics loss, OpenStreetMap
  tiles and touch gestures.
- **Live providers**: Immich photos, Open-Meteo weather and a real Backlight. This phase
  sent no command to any device and made no deployment.
- **Readiness**: reviewed, pull request, merged and physically accepted are separate
  states; none is implied by this document or by a passing `just dashboard-check`.
