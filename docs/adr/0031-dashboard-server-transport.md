# Dashboard server transport

The TanStack Start server is the Dashboard's only path to Axum (ADR 0003). It
holds the credential, bounds every call and decides what the browser may see.
These rules keep that hop private and honest; the code alone cannot show them.

- **Credentials stay on the server.** The API credential is wrapped in request
  headers in `dashboard/src/lib/openhome.server.ts` and only `fetch` receives
  it. Private runtime configuration also stays on the Start server. The Axum
  base URL keeps an installer-supplied path prefix, with or without a trailing
  slash.
- **Errors are sanitised, never relayed.** Transport and parse errors can carry
  private URLs, catalogue diagnostics or provider messages. They are never
  serialised to the browser or logged. Each `*.server.ts` module catches them
  and returns a bare `unavailable` or `failed` value. Provider messages never
  become observed device state or user-facing copy.
- **An acknowledgement is not state.** A command acknowledgement never
  establishes device state. A write counts as confirmed only when Axum's
  parsed answer confirms it. A malformed acknowledgement cannot confirm a
  saved Binder entry or addition, a goal write or a persisted Route Loop
  selection, and only safe protocol values are read from responses. A failed
  transport or RPC never becomes a fabricated route, and comparing Loops never
  writes.
- **Unavailable is not empty.** A photo transport failure never means the
  library is empty: it becomes an `unavailable` read with reason
  `provider-unavailable`, never a `ready` read with no photos. A null nutrient
  amount is unknown, never zero.
- **Dashboard timeouts outlast Axum's.** The dashboard's timeout on each call
  must exceed Axum's own upstream limit, so Axum's outcome can come back. IR
  and SwitchBot commands get 12 s against Axum's 10 s. IR discovery gets only
  6 s, so a stalled IR bridge does not leave remote cards on Checking. Route
  Loop requests leave room for Axum's 20 s attempt window, with 10 s per
  attempt. The backlight change gets 65 s. In the browser,
  `use-single-command.ts` allows 15 s, which covers the server's 12 s and
  also bounds a dropped iPad connection.
- **Axum owns computation and validation.** Axum computes nutrition targets,
  micronutrient panels and summaries, Important Date figures and Recovery Day
  scores. The dashboard only formats them and never recomputes those figures.
  Axum owns profile validation, null clearing and duplicate-date conflicts, so
  the dashboard forwards only supported writes. Typed Workout writes rule out
  body-shape 422s, so a 422 means an entry named an Exercise that is not stored.
  Axum's progress lines carry a unit, but the metric implies it, so the dashboard
  drops it.
- **Client validation mirrors Axum exactly.** List text is trimmed with Rust's
  `str::trim` semantics (Unicode White_Space) in `dashboard/src/lib/lists.ts`.
  JavaScript's `trim` differs, so the client and the API would otherwise
  disagree. The profile editor shows a supported sex in canonical spelling,
  because Axum reads it trimmed and case-insensitively. Any other stored text is
  shown unchanged and stays selectable until the user changes it.
- **Reads are bounded.** Body history reads cap at 500 rows. The Workout
  Log reads one extra row to learn whether older Workouts exist, because there
  is no count endpoint. A Workout removed between the list and detail reads is
  left out rather than failing the Log. Axum scores one Recovery Day per read,
  so the day strip reads the days around the selection: it opens on a month
  around it and fills toward today and the past.
- **Relayed bytes are bounded.** Photo and meal photo relays forward one bounded
  body and never pass on provider headers, redirects or error bodies, even from
  an older or misconfigured Axum. A 1,600 px AVIF is a few hundred KB, so a
  meal photo body above the 5 MB cap is not one of Axum's photos and is
  rejected. A meal photo ID always names the same image and a replaced photo
  gets a new ID, so its relay is cached as immutable with no version in the
  address. The wish picture relay accepts SVG, which only the synthetic preview
  (ADR 0019) serves; Axum itself relays raster pictures alone. An unsaved wish
  picture is never fetched from the shop by the browser.
- **Paths cannot be steered.** URL handling normalises literal dot segments
  even when they are percent-encoded. A NOOP strap identity that contains them
  is therefore rejected, so it cannot alter the Axum path.
- **The sun uses a public point.** Sun position is computed for a city-level
  point in central Copenhagen, never the household's private location.
