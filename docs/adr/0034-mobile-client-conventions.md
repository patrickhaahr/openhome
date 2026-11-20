# Mobile client conventions

The native Android app speaks to Axum for both Home controls and fitness data.
These rules cover the contract details and platform quirks that its code relies
on without showing why.

- **Home commands are not queued.** Home calls share one HTTP client with a 5 s
  total call timeout and no connection retry (`AppContainer.homeHttpClient`); a
  household command is never retried automatically, because a repeat could act
  twice. Each Home view model owns one in-flight command per control group.
- **Acknowledged is not state.** `Acknowledged` means Axum accepted the request,
  never that the device state is known, and an acknowledgement never substitutes
  for a readback. Axum exposes explicit Light commands with no power or capability
  read. A Backlight color or brightness draft sends nothing until its key is
  pressed.
- **Backlight `enabled`.** Axum treats the presence of `enabled` in a Backlight
  bedtime update as an explicit request to clear the manual override. A time or brightness
  edit therefore omits it, even when its value is unchanged.
- **Fitness values.** Dates are `YYYY-MM-DD` calendar days, never shifted through a
  time zone, and `null` means absent, never zero. The birth date range ends at
  today in Copenhagen, wherever the phone is. A stored zero Added Weight in old logs
  reads as blank, the same as bodyweight.
- **Sets and Exercises.** Reps or Timed Hold is stored on each Set, not on the
  Exercise; the logging and correction screens switch every row of an entry at
  once. A past Set may hold both, and Repeat keeps both. Last-logged values come
  from what was actually logged, never from the Exercise Category.
  Exercise names that differ only by case are refused. The API allows an entry
  without Sets, but logging requires a row. Decimal commas are accepted because
  phone keyboards type them. A new Body Weight on an already recorded date is a
  conflict.
- **Appearance.** System bars are always dark, because the app has no light
  appearance. A word replaced by a symbol stays available to assistive technology
  through a 1 dp element, because Android omits nodes with no size.
- **Build.** The devenv Android SDK is read-only, so AGP cannot fetch another
  version and the SDK level follows devenv's. Device tests clear app data, so the
  debug build has its own application id to keep them away from a real install.
