# Shared Watchlists through the Dashboard and OpenHome MCP

Each configured Household User has one shared-visible Watchlist. Axum owns
storage, validation, destination checks and retry identity. Household User IDs
come from the existing `/api/household-users` discovery; configure members in
Dashboard Wishlist. Ownership does not introduce private accounts or change
the existing shared fitness model ([ADR 0004](adr/0004-household-users-and-list-ownership.md)).

This implements manual planning in [#101](https://github.com/patrickhaahr/openhome/issues/101).
Watching/completion transitions and editable dates remain in #102; TMDB search,
metadata and attribution remain in #103. No provider or viewing history is used
to infer personal state.

## Operations

Every operation requires `Authorization: Bearer <API_KEY>`. Dashboard holds this
credential on the TanStack Start server. MCP forwards one authenticated Axum
request per tool and returns its JSON unchanged.

| MCP tool | Authenticated Axum operation | Input |
| --- | --- | --- |
| `household_users` | `GET /api/household-users` | None; discovers all owners, including empty watchlists |
| `watchlist_entries` | `GET /api/watchlist/entries` | None; shared read across all owners |
| `add_watchlist_entry` | `POST /api/watchlist/entries` | `kind`, `title`, `request_id`, explicit `owner_id` |
| `watchlist_entry` | `GET /api/household-users/{owner_id}/watchlist/entries/{entry_id}` | Explicit owner and entry IDs |
| `delete_watchlist_entry` | `DELETE /api/household-users/{owner_id}/watchlist/entries/{entry_id}` | Explicit owner and entry IDs |

Migration 0009 persists each entry's stable UUID `id`, `owner_id`, `kind` (`movie`
or whole `series`), `title`, `status` (`planned`) and `date_added` (the Axum server's
current Europe/Copenhagen calendar date, `YYYY-MM-DD`). Startup applies migrations;
local development can run `devenv shell -- just api-migrate`.

Titles allow 1–500 characters and request IDs allow 1–128; both are trimmed using
Unicode whitespace and reject control characters. Unknown body fields, episode
kinds and supplied status/date fields are rejected. Missing or mismatched
owner/entry destinations return 404. A successful removal returns 204, mapped to
MCP `{ "deleted": true }`; repeated removals remain not-found tool errors.

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
Only a parsed API receipt clears it. Removal requires confirmation, names its owner,
and refreshes from Axum; a failed removal keeps the dialog open. Incomplete reads
show recovery controls instead of an empty household. After an uncertain removal,
read its destination before retrying.

Reads are annotated read-only; deduplicated additions are non-destructive and
idempotent; removals are destructive and idempotent. All training tools retain
their read-only contracts. API and transport failures are MCP tool errors and
never imply a successful write.

## Verification

Run `devenv shell -- just api-check mcp-check dashboard-check`. The primary domain
proof is migrated SQLite behind authenticated Axum in `api/tests/watchlist.rs`.
The real Streamable HTTP MCP client/adapter/Axum seam is in `mcp/tests/contract.rs`.
Rendered Dashboard tests use the same authenticated routes with synthetic owners,
including additions/removal, refresh, failed credentials and a lost accepted
response followed by remount/retry. [Public screenshots](../dashboard/docs/watchlist/README.md)
show synthetic data only. Physical iPad Safari validation remains a separate
release gate under #69.
