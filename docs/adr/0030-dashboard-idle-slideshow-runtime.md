# Idle Slideshow runtime

The Idle Slideshow, or the Night Screen from 23:00 to 08:30, covers the
Dashboard after inactivity and must hand the room back exactly as it was (see `docs/context/dashboard.md` and `docs/photos.md`).
Its idle, wake and playback owners are recorded in `docs/dashboard-lifecycles.md`.
Its timers, listeners and photo reads race with touches and with each other, so
these rules hold.

- **The wake gesture is one tagged union.** The Night Screen and the Idle
  Slideshow share it: `none`, `held` while a pointer or key that woke the cover
  is down, and `trailing` after the last held contact is released or cancelled.
  A held key stays swallowed, with its repeats, `keypress` and `keyup`, even
  after the page is showing, so Space cannot activate a button on its `keyup`
  and Enter cannot repeat into a control. The 450 ms grace runs only in
  `trailing`, and each swallowed trailing event restarts it, so a late
  compatibility click hit-tested to the key beneath is still eaten. A release
  lost for 3 s shows the page but keeps `held`, so that release stays
  swallowed, and the next primary press or a different key settles it.
- **Inactivity is measured on the monotonic clock.** Activity is stamped with
  `performance.now()`, so moving the wall clock cannot stop or rush the cover.
  Whether the cover is the Night Screen is read from `now()` when the timer
  fires, not from the last clock poll.
- **The crossfade survives reduced motion.** The reduced-motion rule in
  `styles.css` zeroes every transition with `!important`, which no utility can
  outrank, so it excludes the idle slide and the slideshow keeps an opacity-only
  crossfade.
