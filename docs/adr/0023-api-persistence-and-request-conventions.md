# API persistence and request conventions

The Axum API keeps three SQLite databases and many small write endpoints.
`docs/api-receipts.md` records the retry-receipt outcomes per domain. The
conventions below cut across those domains and are not visible from any one
route.

- **Three databases, runtime-checked queries.** `app.db` holds household data.
  `noop.db` holds pushed health data, and `food.db` holds rebuildable reference
  data. The sqlx compile-time macros check only against `DATABASE_URL`
  (`app.db`), so queries against `noop.db` and `food.db` use the
  runtime-checked `sqlx::query`. `FOOD_DB_URL` overrides the food database
  path; otherwise it is `DATABASE_URL` with the file name swapped.
- **Why `noop.db` is separate.** Large push batches never hold the main
  database's write lock. Received health data is the only copy outside the
  phone, so it needs its own backup schedule. The store can be rotated or reset
  without touching `app.db`.
- **Push authentication.** The NOOP push route uses its own `NOOP_PUSH_TOKEN`,
  so its router is merged outside the API-key layer. Every route that reads
  private data is merged inside it. The token is checked before any of the body
  is read. The up-to-4 MiB decode, parse and hashing run on a blocking thread,
  so they cannot starve the async workers. Startup rejects a token with
  leading or trailing whitespace (often left by a quoted `.env` value), because
  it could never match and would answer 401 forever.
- **Lenient push numbers.** The push protocol does not type `aux1Raw` or
  `aux2Raw` as integers, and the sender picks the JSON type from SQLite storage
  type. Values such as `12.0` therefore arrive, so these columns are REAL and
  fractional values never stall a stream.
- **Write lock first.** Retry-safe creates take SQLite's write lock with their
  first statement, either the receipt claim or the first insert. Simultaneous
  retries then serialise there, and the loser finds the winner's receipt. Push
  batches use `BEGIN IMMEDIATE` for the same reason.
- **Receipts survive schema growth.** A receipt's comparable arguments omit
  absent or empty fields: no photo hash without a photo, no doses without
  doses, and `[food_id, grams]` for a Food portion without estimates. A field
  added later cannot make an earlier receipt mismatch its own retry. A Meal
  Log made by Meal name stores the lowercased name and overrides, so a retry
  replays after the Meal is edited.
- **PATCH distinguishes null.** PATCH bodies must tell an absent field from an
  explicit null. serde reports a present null through `visit_none`, and an
  absent field never reaches the deserializer. Fitness routes therefore use the
  `double_option` deserializer (`routes/fitness.rs`), and nutrition uses
  `Patch<T>` (`services/patch.rs`), which defaults to `Missing`. Both map absent
  to keep, null to clear and a value to set.
- **Text dates sort.** Dates are stored as zero-padded `YYYY-MM-DD`, even though
  chrono parses unpadded fields. `CalendarDay::parse` rejects any other
  spelling, and fitness dates are re-formatted into the padded form before they
  are stored. List ordering and `from`/`to` filters compare strings
  lexicographically.
- **Copenhagen calendar days.** Days are bounded by local midnights in the
  IANA zone, so a day lasts 23 or 25 hours at DST changes and nothing assumes
  24 hours. Copenhagen never skips midnight; in a zone that did,
  `CalendarDay::start` would read the missing local midnight as UTC. Tests that span 2026-03-29 guard this.
- **Reject, never truncate.** Reads over a row or byte cap fail rather than
  return a partial answer. Row caps do not bound free-text names, so the
  Training Context also caps serialized bytes.
- **Test databases.** A test of a lock refusal or of concurrent readers needs
  a real temporary `app.db` file. A `:memory:` pool gives each connection its
  own database, and a second writer cannot lock it. Response floats round-trip
  through JSON text, and serde_json's default parser can land one ulp from the
  exact quotient, so tests compare floats with a tolerance.
- **NOOP parity details.** OpenHome ports NOOP's read-side rules so it selects
  what NOOP shows (`docs/training-context-rules.md`). Two details are its own.
  The computed (detector) device id also contains `whoop`, so classification
  checks for the detector before the WHOOP-import match. NOOP does not push its
  Rest score, so OpenHome computes it with NOOP's constants. Its history
  differs from NOOP's: the window ends at the scored day instead of the present
  and covers at most 90 days. Each local day contributes its longest session
  times its efficiency (0.9 when missing), instead of NOOP's duplicate
  collapsing.
