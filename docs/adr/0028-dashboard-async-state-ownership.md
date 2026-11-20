# Dashboard async state ownership

Dashboard writes and reads settle after the view that started them has moved
on: sheets close, effects re-run and Await re-suspends. These rules decide who
owns pending work and what a late result may still do. Each lifecycle's owner,
exits and liveness limits are recorded in `docs/dashboard-lifecycles.md`.

- **State lives above what it outlives.** Drafts and choices are held above any
  view that a read can suspend or replace. The Profile edit draft sits above
  the read-dependent view, so a read outage cannot discard unsaved input. The
  exercise comparison toggles and period choices survive period changes and
  added Exercises. The IR remote's pending-command guard sits above Await, so a
  re-check that suspends discovery cannot reset it. Each check resolves a new
  discovery, because an earlier outcome no longer describes the current buttons.
- **Drafts restart from what was read back.** The Binder entry draft starts from
  the stored entry, and the parent remounts it whenever that entry changes. A
  saved edit therefore shows what was read back, and a failed one keeps the
  draft. Removal asks first, and only a saved removal closes the sheet. The food
  goal choice resets whenever the served goal changes, for example when it could
  not be read at first. It adjusts state during render so the save feedback
  survives.
- **A completion may only touch its own sheet.** The board's save hook outlives
  its sheets, and its `done` closes whichever sheet is open, so a late
  completion from a closed sheet would otherwise close a different one.
- **Same-tick double submits are blocked synchronously.** Reducer state lags a
  render, and taps can queue before React renders disabled buttons. A ref guards
  saves (`list-parts.tsx`) and commands (`use-single-command.ts`) within one tick.
- **Refetch follows the page, not the clock.** Food re-reads at Copenhagen
  midnight while it follows today; Trends always end today. Photos and Binder
  re-read on entering the page or choosing an album, but never on in-page
  navigation. Photos and Binder use `gcTime` 0.
