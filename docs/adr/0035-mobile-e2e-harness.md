# Mobile end-to-end harness

`mobile/e2e` drives the real Android app on an emulator against a fake Axum.
These rules keep the tests faithful to production and stable against Compose and
Android timing.

- **Separate process.** The e2e module is a self-instrumenting test app in its own
  process, so a test can force-stop the app and observe a real relaunch.
- **FakeAxum mirrors Axum.** `FakeAxum` must follow the Axum fitness contract: page
  limit default 100 clamped to 1..500; Workouts newest first with inclusive `to`;
  Body Weight oldest first with inclusive `from`; a repeated `request_id` answers
  200 and changed arguments 409; Exercise names unique (409); edits apply only for a
  newer `client_revision`. A divergence would let the app pass against a server
  that does not exist.
- **Paging proof.** The Setup Flow fixture places its oldest weigh-in beyond every
  default page, so the test proves Setup walks the full list.
- **Offline means mobile data.** Turning Wi-Fi off leaves the emulator's metered
  mobile data as the default network. The offline tests rely on that to prove the
  Wi-Fi-only sync policy, not merely an absent network.
- **Driver retries.** Compose replaces list nodes during animations and filter
  results arrive from storage asynchronously, so `AppDriver` retries reads, waits
  for absence and flings until the list is still. Vibrations are recorded only when
  they end.
- **Exit transitions.** The departing screen stays in the accessibility tree through
  its exit transition, so `assertNoBottomTabs` waits until no tab is selected before
  concluding the bar is gone.
- **Home timeout.** Home calls end at the 5 s call timeout, so tests observe held
  commands without scrolling and release them at once.
