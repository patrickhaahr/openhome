# Idle Slideshow runtime

The Idle Slideshow covers the Dashboard after inactivity and must hand the room
back exactly as it was (see `docs/context/dashboard.md`). Its timers, listeners
and photo reads race with touches and with each other, so these rules hold.

- **The page is hidden, not unmounted.** Under the slideshow the page stays
  mounted as it was but is made unreachable. Scroll position and drafts
  therefore survive waking. Focus is remembered by the showing phase and handed
  back once.
- **Phase and blocks are separate facts.** An Idle Block is a reason not to
  enter idle, never a phase. Several can be held at once, and each is released
  on its own. One reducer in `dashboard/src/components/idle.tsx` is the only
  transition boundary: timers and listeners only report events. A callback that
  outlives its cause, such as a countdown after a block, is a no-op.
- **Waking swallows the whole gesture.** The first touch or key only wakes, and
  nothing reaches controls beneath. The browser hit-tests the slideshow, so a
  late compatibility click could land on the key beneath; the wake tap eats even
  a stray retargeted click. One settle timer restarts on each trailing event of
  the gesture. It is keyed on showing rather than the exact phase, so the timers
  keep running from idle to waking.
- **A lost release cannot leave the room asleep.** A held finger wakes after a
  timeout.
- **Each playback run owns its generation.** Every effect run owns its playback,
  and its `AbortSignal` is its generation. Once aborted, a run writes neither the
  screen nor the remembered position. Local waits settle on abort. The browser's
  server-function page read is neither cancelled nor bounded, so an old run can
  stay pending, and its late result or failure is ignored. Image load waits are
  keyed, so an old image event cannot settle a new load. A replacement run starts
  with empty slots rather than inheriting photos.
- **The crossfade is opacity only.** It keeps `!important` in `styles.css`,
  because no utility can outrank it.
- **Instant idle is preview-only.** `?idle=now` starts the slideshow at once
  only when `IDLE_PREVIEW=true`, which only a synthetic preview stack sets. The
  browser never reads the environment itself. `IDLE_TIMEOUT_SECONDS` accepts 10
  to 86400, 0 disables idle, and any other value falls back to 5 minutes.
- **Only allowed dates reach the room.** The idle display reads only Important
  Dates with Show When Idle enabled, because anyone in the room can see it.
