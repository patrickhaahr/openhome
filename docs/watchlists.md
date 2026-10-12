# Shared Watchlists through the Dashboard and OpenHome MCP

Each configured Household User has one shared-visible Watchlist. Axum owns
storage, validation, destination checks and retry identity. Household User IDs
come from the existing `/api/household-users` discovery; configure members in
Dashboard Wishlist. Ownership does not introduce private accounts or change
the existing shared fitness model ([ADR 0004](adr/0004-household-users-and-list-ownership.md)).

Manual planning is delivered in [#101](https://github.com/patrickhaahr/openhome/issues/101);
personal viewing states, date corrections and filters in [#102](https://github.com/patrickhaahr/openhome/issues/102).
TMDB search, metadata and attribution are delivered in [#103](https://github.com/patrickhaahr/openhome/issues/103). No provider or viewing history is used
to infer personal state.

## Operations

Every operation requires `Authorization: Bearer <API_KEY>`. Dashboard holds this
credential on the TanStack Start server. MCP forwards one authenticated Axum
request per tool and returns its JSON unchanged.

| MCP tool | Authenticated Axum operation | Input |
| --- | --- | --- |
| `household_users` | `GET /api/household-users` | None; discovers all owners, including empty watchlists |
| `watchlist_entries` | `GET /api/watchlist/entries` | Optional `owner_id` and `status` filters; omitted filters read across all owners/statuses |
| `add_watchlist_entry` | `POST /api/watchlist/entries` | `kind`, `title`, `request_id`, explicit `owner_id` |
| `watchlist_entry` | `GET /api/household-users/{owner_id}/watchlist/entries/{entry_id}` | Explicit owner and entry IDs |
| `update_watchlist_entry` | `PATCH /api/household-users/{owner_id}/watchlist/entries/{entry_id}` | Explicit owner/entry IDs; optional `status`, `date_added`, `completion_date` |
| `delete_watchlist_entry` | `DELETE /api/household-users/{owner_id}/watchlist/entries/{entry_id}` | Explicit owner and entry IDs |

Migration 0009 persists each entry's stable UUID `id`, `owner_id`, `kind` (`movie`
or whole `series`), `title`, `status` (initially `planned`) and `date_added` (the Axum server's
current Europe/Copenhagen calendar date, `YYYY-MM-DD`). Migration 0010 adds nullable
`completion_date` and enforces completion/status consistency in SQLite. Legacy
completed rows are backfilled from their added date before enforcing the constraint.
Rolling back 0010 discards viewing progress and completion dates, resetting entries
to planned for compatibility with the previous application; IDs, added dates and
addition receipts remain intact. Back up before a downgrade (`just api-migrate-revert`). Startup applies migrations;
local development can run `devenv shell -- just api-migrate`.

Titles allow 1–500 characters and request IDs allow 1–128; both are trimmed using
Unicode whitespace and reject control characters. Unknown body fields, episode
kinds and supplied status/date fields are rejected on addition. Optional `tmdb_id`
selects provider metadata; omit it for a manual plan. Missing or mismatched
owner/entry destinations return 404. A successful removal returns 204, mapped to
MCP `{ "deleted": true }`; repeated removals remain not-found tool errors.

## Personal status and dates

`PATCH` changes only supplied status/date fields; title, kind, owner and retry
identity remain unchanged. All transitions between `planned`, `watching` and
`completed` are permitted. Completing records the current Europe/Copenhagen
calendar date unless an explicit `completion_date` is supplied. A repeated
completed transition preserves the recorded date. Leaving completed clears it;
`date_added` is preserved unless explicitly corrected.

Dates must be real `YYYY-MM-DD` calendar dates, years 0001–9999. A completed entry
requires a completion date on or after its added date; correct both together
when needed. Null `status`/`date_added`, null completion while completed, and
non-null completion outside completed are rejected without changing anything.
No rewatch history, episode progress or provider-inferred completion is stored.
Concurrent corrections detected during a PATCH return 409; refresh before retrying.
After an uncertain edit, read the entry before deciding to retry so a newer
correction is not overwritten.

```json
{ "status": "completed", "date_added": "2024-02-28", "completion_date": "2024-02-29" }
```

`GET /api/watchlist/entries?owner_id=<id>&status=watching` combines optional owner
and status filters. Invalid statuses are rejected; unmatched owners produce an
empty filtered list. Dashboard retains owner/status filters in the URL, while
showing configured household members including those without matching entries.
Each entry's **Edit status & dates** form targets its named owner, keeps failed
edits open, and reads back the persisted result after success.

## Ownership and retries

Discover Household Users before adding. Axum never guesses an owner, even when
only one is configured. Omitting `owner_id` returns 409 with
`code: "watchlist_destination_required"`, one `question`, and `choices` containing
Household User IDs/names. Nothing is written and the request ID remains available.
Ask the returned question once, then submit the selected owner ID. An empty
household needs a member configured in Wishlist first.

Generate a unique `request_id` per intended addition. Keep the exact owner, kind,
title and ID on every retry after an uncertain response. Axum commits an immutable
receipt and entry together, including simultaneous retries. The first addition
returns 201 with `{ "entry": { ... }, "replayed": false }`; an identical accepted
retry returns 200 with the original entry and `replayed: true`. Changed arguments
with an accepted ID return 409. A separate intentional addition uses a new ID,
even for the same title. Different owners and movie/series plans are independent.

Receipts survive entry and owner deletion. A delayed retry returns 409 instead of
recreating removed plans. Back up the main database including
`watchlist_addition_receipts`. Failed transactions consume no request ID.

Dashboard saves unconfirmed additions in tab-local session storage before dispatch,
preserving owner, kind, normalized title and request ID across refresh. When browser
storage is unavailable, mounted forms retain the same in-memory retry intention.
Only a parsed API receipt or explicit **Discard draft** clears it. If another
client removes a plan whose addition response was lost, ordinary retries keep its
retired ID and return conflict. Discard the draft and re-enter the title to make
a new intentional addition with a fresh ID. Removal requires confirmation, names its owner,
and refreshes from Axum; a failed removal keeps the dialog open. Incomplete reads
show recovery controls instead of an empty household. After an uncertain removal,
read its destination before retrying.

Removing a Household User in Wishlist also removes all their watchlist entries.
The member-removal dialog discloses both Wishlist and Watchlist deletion; accepted
addition receipts remain retired.

Reads are annotated read-only; deduplicated additions are non-destructive and
idempotent; corrections and removals are destructive and idempotent. All training tools retain
their read-only contracts. API and transport failures are MCP tool errors and
never imply a successful write.

## Verification

Run `devenv shell -- just api-check mcp-check dashboard-check`. The primary domain
proof is migrated SQLite behind authenticated Axum in `api/tests/watchlist.rs`.
The real Streamable HTTP MCP client/adapter/Axum seam is in `mcp/tests/contract.rs`.
Rendered Dashboard tests use the same authenticated routes with synthetic owners,
including additions/removal, status/date edits, filters, refresh, failed credentials and a lost accepted
response followed by remount/retry. [Public screenshots](../dashboard/docs/watchlist/README.md)
show synthetic data only. Physical iPad Safari validation remains a separate
release gate under #69.

## TMDB search, selection and enrichment

Set `TMDB_READ_ACCESS_TOKEN` privately on Axum using TMDB's **API Read Access
Token**, not the v3 query-string API key. Missing or invalid configuration disables
provider reads and leaves manual CRUD available. Never put the token in Start,
`VITE_*`, MCP, issues or captures. The API uses the fixed `https://api.themoviedb.org/3/`
origin, bearer authentication, a three-second timeout, no redirects and a 1 MiB
response limit. Provider error bodies and tokens are never returned to clients.

- `GET /api/watchlist/search?kind=movie&query=...` (or `kind=series`) returns the
  first page, up to 20 typed metadata results. Search movies and TV separately;
  people, episodes and seasons are excluded. Queries require 1–200 characters.
- `POST /api/watchlist/entries` accepts optional positive safe-integer `tmdb_id`.
  Axum resolves the selected kind/ID itself, without trusting client descriptions,
  posters or dates. New selected entries still start **planned** today.
- `POST /api/household-users/{owner_id}/watchlist/entries/{entry_id}/metadata`
  accepts `{ "tmdb_id": 42 }`. It enriches/refreshes the existing entry's title,
  overview, poster path and release date while preserving its kind, owner, stable
  ID, added date, status and completion date. A wrong owner never triggers a
  provider request. Concurrent metadata replacement returns 409.

Migration 0011 adds nullable `tmdb_id`, `overview`, `poster_path` and `release_date`
and a unique `(owner_id, kind, tmdb_id)` index for provider entries. Equal numeric
movie and series IDs remain distinct; each owner can plan the same provider title.
Manual entries need no provider identity. Rollback drops metadata while preserving
personal progress and dates. Addition receipts retain the original requested title
and provider ID; retries resolve before provider reads, even after enrichment or
an outage. Reusing an ID with a different provider selection conflicts. Trying to
add an existing owner/kind/provider title with a new request ID also returns 409.

Configuration, quota, search and detail failures return sanitized 503
`code: "tmdb_unavailable"`, never an empty successful search. The dashboard keeps
the manual title/type form available and provides **Add manually** after selection.
No metadata failure consumes a request ID. Existing MCP tools continue to support
manual plans and return enriched entries unchanged; this slice adds provider
selection and enrichment to the dashboard/API, without new agent tools.

TMDB confirms free non-commercial developer access with attribution in its
[FAQ](https://developer.themoviedb.org/docs/faq). The Watchlist's Credits area shows
an approved logo, link and required notice. Axum validates poster filenames, and
Start builds public `image.tmdb.org/t/p/w342/...` URLs without tokens.
See [ADR 0008](adr/0008-watchlist-tmdb-metadata.md).

Validation uses migrated authenticated Axum routes with synthetic provider
responses, plus rendered selection, persisted refresh, enrichment, attribution
and manual outage recovery. The loopback `dashboard_health` fixture serves a
synthetic upstream; search **Orbit** as a movie or **Lighthouse** as a series, and
**Offline story** to exercise search failure. Its posters are intentionally fictional.
This proves no live TMDB or physical Safari/device behavior. Live authenticated
provider access and target-iPad verification remain release acceptance checks;
no TMDB runtime credential was available for this implementation.
