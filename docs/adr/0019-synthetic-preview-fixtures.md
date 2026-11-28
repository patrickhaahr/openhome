# Synthetic previews and test fixtures

The Dashboard preview (`api/examples/dashboard_preview`), the health seam
(`api/examples/dashboard_health.rs`), the MCP test harness and the mobile
fixture seeder have one purpose: drive production code against invented data.
They never touch a household database, credential or upstream.

- **Production code, fake upstreams.** Each upstream is a loopback stand-in
  that speaks the real wire format, so the production Integration Services run
  unchanged. Household records are seeded through the public HTTP routes, so
  the seed data always matches the contract. NOOP data is pushed through the
  real ingestion protocol. A batch holds at most 5,000 records, so long
  histories go in as a continued stream. Empty windows are real applied
  replacements, and no fixture claims coverage just because a measurement
  exists.
- **Nothing real.** Photos are drawn scenes with no people or places, and wish
  images are drawn too. Food values are invented, not DTU values. The
  synthetic barcode is a restricted-circulation GS1 number, so it never matches
  a real package. The map and weather use the public Copenhagen demo origin.
  The preview API key is not secret: it only authorises the loopback fixture.
- **No startup traffic to real services.** Watchlist posters use static TMDB
  ids and paths from the seed. Adding entries by TMDB id would call TMDB once
  per entry. Real TMDB is used only when `TMDB_READ_ACCESS_TOKEN` is set. The
  OPTCG catalogue is never contacted: it is one volunteer's server (ADR 0017).
- **The same output on every run.** The generators are seeded (an LCG and
  splitmix64), so a restart shows the same nights and photos. Data is
  generated relative to today, so the current day always has content. Times
  are pinned to noon UTC so each photo stays on its day in Copenhagen. A
  29 February date falls on the 28th in common years so that "on this day"
  still finds it. A bedtime after noon belongs to the evening before the wake
  day.
- **Harness details.** The stand-ins log their address without a scheme,
  because test harnesses take the first `http://127.0.0.1:<port>` they see to
  be Axum's URL. In-memory SQLite pools keep one connection with no idle
  timeout, because each in-memory connection is its own database. The wish
  picture stand-in is mounted inside the API-key layer, because the production
  fetcher refuses loopback addresses. Synthetic UUIDs keep the version 4 and
  RFC 4122 variant bits: the low 48 bits carry album and photo numbers, and
  bit 48 tells albums from photos. Every tenth mobile fixture Workout shares a
  date with the one before, so page boundaries fall inside a day.
- **Knobs.** The `OPENHOME_PREVIEW_*` variables and `--food-database=importing`
  are documented in the verify-openhome skill. One more,
  `OPENHOME_PREVIEW_ALBUMS=<n>`, limits the selected Immich albums to the first
  *n*. `OPENHOME_PREVIEW_COMPLETE_TODAY=true` seeds today's sleep, naps and
  workouts in full instead of ending them at the moment of the start, so a
  capture made at night still shows a finished day. After the price-history
  replay, the preview waits 25 s on the first scheduler start (nine codes at a
  two-second pace) and 3 s on later starts.
- **verify-openhome runs.** Runs and evidence live outside the checkout, so
  they survive worktree removal. The stack starts under `setsid`, so `down.sh`
  stops exactly the process group that run started.
