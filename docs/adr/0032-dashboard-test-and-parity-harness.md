# Dashboard test and parity harness

Dashboard tests run in happy-dom against the real server transport. The visual
parity harness in `dashboard/tests/parity/` compares the working tree with
master in Chromium against the synthetic preview (ADR 0019). Both work around
environment gaps that the tests themselves do not explain.

- **Real transport, no CORS.** The fixture supplies the real authenticated
  transport behind Start's RPC boundary. Tests that run it set happy-dom's
  `disableSameOriginPolicy`, because the transport calls Axum directly and
  browser CORS applies only to Start RPCs.
- **happy-dom differs from a browser.** It keeps cookies across tests, so a
  test that depends on a cookie expires it explicitly. Dispatched pointer events
  do not produce the click that a real browser fires after `pointerup`, so
  tests click explicitly. Text input goes through the native value setter, so
  React's tracked value is not replaced and React sees the change.
- **No WebGL in tests.** happy-dom has no WebGL context. Only the renderer
  probe's detached canvas is granted WebGL2, so MapLibre is chosen and then
  finds no context on its attached canvas. The probe only reads `getExtension`.
- **Forms are tested at the seam.** Add-form tests dispatch at the form, so Axum
  validation runs even where the browser's native URL check would stop it.
- **Intl spacing is literal.** Intl puts a no-break space between a currency
  code and its amount, so expected strings use it.
- **Parity targets survive the migration they guard.** Scenario targets use
  roles and visible text only, never class names, because the styling migration
  under test removes class names. One viewport is short enough to trigger the
  stylesheets' `max-height` media queries.
- **Both sides see the same instant and data.** The synthetic data is relative
  to the real clock, so both sides are captured at one fixed instant. The
  preview runs with an empty TMDB token, so it uses its loopback catalogue and
  never the live service.
- **Pixels have no tolerance.** Any RGBA difference counts, so the noise floor
  is zero. A capture with a non-zero diff is compared a second time.
  Request-time data, such as the hour strip or check time, can straddle a
  boundary between the two sides. Partial raster repaints also draw image tiles
  over stale ones, leaving a few rounded-corner pixels different between
  identical renders.
- **Diffs inform; only untrusted captures fail.** People judge the diffs. The
  run fails only on a capture the harness could not trust.
- **Every browser wait is bounded.** A lazy image scrolled past within one frame
  may never load, which would stall `imagesReady`. `decode()` on an image that
  never started waits forever. Any browser call without its own timeout is
  bounded, so no single call can stall the run.
