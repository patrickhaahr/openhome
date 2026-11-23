# Watchlist

Each Household User has a Watchlist of movies and whole series. Hermes adds titles for an explicit owner, a retried addition replays instead of duplicating, and the Dashboard's Lists → Watch view shows every member's titles grouped by status.

## Sub-features

- `watch-add-mcp` adds a manual title through `add_watchlist_entry` for an explicit owner.
- `watch-replay` repeats the same `request_id` and arguments and gets the original entry with `replayed: true`.
- `watch-tmdb-mcp` finds a title with `search_watchlist_titles` and adds it with its `tmdb_id`, so the entry comes back with overview, poster path and release date.
- `watch-owner-required` omits `owner_id` and gets a clarification error with choices, with nothing written.
- `watch-dashboard` shows the title in the Dashboard's Planned group with its owner's initial.

## How to get to it (user POV)

- Ask Hermes to add a movie or series to someone's Watchlist (MCP tools `household_users`, `add_watchlist_entry`, `watchlist_entries`).
- Dashboard: `Main navigation` → `Lists` → the `Watch` tab, filtered with the `Whose list` group.
- Dashboard: the `Add a title` button.

## Driving it with verify-openhome

Preconditions:

- `doctor.sh` prints `OK`.
- No entry titled `Verify Harbour Lights` exists (true after a fresh launch).

- **Find the owner.** Run `mcp.sh call household_users '{}'`. `structuredContent.items` lists Alex, Robin and Sam with IDs; take Alex's.
- **Add.** Run `mcp.sh call add_watchlist_entry '{"owner_id":"<alex>","kind":"movie","title":"Verify Harbour Lights","request_id":"verify-<run id>"}'`. The result has `replayed: false`, `status: "planned"` and today's `date_added`.
- **Retry.** Run the identical call again. The result has `replayed: true` and the same entry `id`.
- **Read back.** Run `curl -fsS -H "Authorization: Bearer $API_KEY" "$API_URL/api/watchlist/entries?owner_id=<alex>"`. The list holds exactly one `Verify Harbour Lights`.
- **Enriched add.** Run `mcp.sh call search_watchlist_titles '{"kind":"movie","query":"Orbit"}'` (loopback catalogue), take `structuredContent.items[0].tmdb_id`, then run `mcp.sh call add_watchlist_entry` with it as `tmdb_id` plus a fresh `request_id`. The entry has the catalogue's `title`, `overview`, `poster_path` and `release_date`; `watchlist_entry` and the Axum read-back show the same values.
- **Dashboard.** `preview_open` on the Dashboard port with path `/lists?tab=watch`, then `preview_wait_for` text `Verify Harbour Lights`. The `Planned` region's list shows the title, and the `Whose list` button reads `Alex 7 titles to watch` (5 seeded planned or watching + the 2 added).
- **Proof.** Save the tool JSON as `01-…`–`04-…` and a `preview_snapshot` with `save: true` copied to `05-dashboard-watch.png` under `$EVIDENCE_DIR/watchlist/`.

## Gotchas

- `household_users` IDs are regenerated on every launch. Never hard-code them.
- Seed data already has `Spirited Away` for two owners, so use `Verify…` titles to avoid ambiguous assertions.
- TMDB search uses a loopback catalogue unless `~/.config/openhome/tmdb-read-access-token` exists, in which case it hits live TMDB. Search **Orbit** (movie) or **Lighthouse** (series) only with the loopback catalogue.
- The Dashboard reads on navigation. After an MCP write, open or reload the page instead of expecting a live update.
