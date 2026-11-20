# Dashboard async state ownership

Dashboard writes and reads settle after the view that started them has moved
on: sheets close, effects re-run and Await re-suspends. These rules decide who
owns pending work and what a late result may still do.

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
  its sheets, and its `done` closes whichever sheet is open. Only a completion
  whose own sheet is still mounted may call `done`. Durable work, such as
  clearing a spent retry intention, runs either way. A lost response is
  reported as uncertain, never as saved.
- **Same-tick double submits are blocked synchronously.** Reducer state lags a
  render, and taps can queue before React renders disabled buttons. A ref guards
  saves (`list-parts.tsx`) and commands (`use-single-command.ts`) within one tick.
- **Stale results are dropped by identity.** Each request, timer or effect run
  carries an identity, and a result from replaced or unmounted work does
  nothing. The photo wall keeps one request id, the only one allowed to settle,
  and clears it on unmount. A retry asks for the failed cursor again.
- **Backlight delivery is one tagged union.** The delivery state in
  `dashboard/src/components/backlight-delivery.ts` is the only record of what is
  waiting or in flight. A single trailing target replaces any queued action or
  earlier edit, even while a request is in flight. An older response must not
  replace feedback for a newer waiting edit. Debouncing ends by timer, by an
  immediate action, or by unmount. Sending ends by response or rejection, then
  delivers whatever waits; unmount drops the response.
- **Server settling is proved; the browser hop is not.** On the server,
  `changeBacklight` always settles: a 65 s abort, and every failure becomes
  `unavailable`. The browser's call and the `router.invalidate` after it have no
  bound. If either never settles, sending stays busy and later actions queue
  behind it. This gap is accepted, not fixed with a guessed timeout.
- **Refetch follows the page, not the clock.** Food re-reads at Copenhagen
  midnight while it follows today; Trends always end today. Photos and Binder
  re-read on entering the page or choosing an album, but never on in-page
  navigation. Photos uses `gcTime` 0.
