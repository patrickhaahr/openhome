# Idle Slideshow runtime

The Idle Slideshow covers the Dashboard after inactivity and must hand the room
back exactly as it was (see `docs/context/dashboard.md` and `docs/photos.md`).
Its idle, wake and playback owners are recorded in `docs/dashboard-lifecycles.md`.
Its timers, listeners and photo reads race with touches and with each other, so
these rules hold.

- **Waking is one settle timer.** The wake event starts a single settle timer,
  and each trailing event of the same gesture restarts it. It is keyed on
  showing rather than the exact phase, so the timer and listeners keep running
  from idle to waking. A late compatibility click can be hit-tested to the key
  beneath; the wake tap eats even a stray retargeted click.
- **The crossfade survives reduced motion.** The reduced-motion rule in
  `styles.css` zeroes every transition with `!important`, which no utility can
  outrank, so it excludes the idle slide and the slideshow keeps an opacity-only
  crossfade.
