# Mobile Active Workout and rest timer

The native Android app logs a Workout while the phone may sleep, restart or kill
the process mid-set. These constraints keep logged work and the rest timer honest
through all of that.

- **One stored Active Workout.** At most one Active Workout exists, and it is
  written on every change. `ActiveWorkoutRepository` serializes writes in call
  order, so fast typing cannot save out of order and Finish sees every prior edit.
  Each change is one transaction, so an interruption leaves the prior state.
  Leaving the screen keeps the Workout, because it lives in storage, not in the
  screen. Start does nothing when one already exists.
- **Confirmed rows only.** A row can be confirmed only while its values parse, and
  an edit that stops it parsing unconfirms it. Finish records only confirmed rows,
  leaves unchecked rows out only when the user chose to omit them, and queues the
  sync and deletes the Active Workout in the same transaction. Repeat copies name,
  order and set values as unconfirmed rows, so nothing is recorded unseen.
- **Complete Exercise.** It confirms the rows that parse, moves focus to the next
  unconfirmed Exercise (wrapping), and starts one rest, or none if nothing was
  confirmed. Unchecking a row never starts a rest.
- **Double-tap safety.** The next Exercise appears under the same Complete
  Exercise button, so that button ignores taps for 400 ms after an Exercise comes
  into view; the second tap of a double tap must not confirm sets the user has not
  seen.
- **Saves outlive screens.** Saves launch in the app container's scope, which
  outlives every screen, so leaving after pressing Save cannot cancel the save. The
  Profile and correction forms stay marked saved afterwards to prevent a second
  save.
- **Wall-clock deadline.** The rest timer stores only a wall-clock deadline, the one
  clock that survives a phone restart; remaining time is computed from it, so
  nothing depends on a live process. Readings round up to the next whole second,
  and the longest reading so far stands as the rest's length.
- **The row owns the rest.** The stored timer row is running or over, and an over
  rest is never revived. Android's single alarm only mirrors it through `syncAlarm`,
  which is idempotent and rerun on app open, because Android drops
  alarms on force-stop and restart. Exact alarms fall back to inexact when the
  permission is revoked, since a late alert beats none. An alarm more than a minute
  past its deadline stays silent, and one that fires before it, as when the wall
  clock moved backwards, is set again.
- **Rest scope.** The rest key is the Active Workout id, so ending the Workout ends
  the rest. A new confirmation replaces a running rest; Exercises with rest turned
  off leave a running rest alone.
- **Vibration.** The alerted flag is written before vibrating, so a failure loses a
  vibration rather than repeating one. Vibration uses the deprecated API because it
  is the one call available on every supported Android version, with alarm usage so
  it still fires in the background. If storage fails, the screen still shows the
  rest as over.
- **Navigation.** One back stack starts at Workout. The tab bar hides during an
  unsaved correction so a tab tap cannot drop it, and finishing or discarding a
  session replaces its destination so Back cannot return to it.
