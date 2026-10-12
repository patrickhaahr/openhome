# Shared Wishlists through the Dashboard and OpenHome MCP

Axum owns Household Users, named Wishlists, generic Wishlist Items and destination
resolution ([ADR 0004](adr/0004-household-users-and-list-ownership.md),
[ADR 0005](adr/0005-wishlist-writes-through-mcp.md)). All reads are household-visible.
Ownership checks select the intended destination; they do not introduce private
accounts or change fitness ownership. MCP forwards one authenticated API request
per tool and returns the JSON unchanged.

## Provisioning and discovery

Run `devenv shell -- just api-migrate` for local development; API startup applies
migrations automatically. Provision a Household User with authenticated
`POST /api/household-users`, body `{"name":"Member A"}`. The response contains a
stable UUID `id` and the trimmed name. Names are unique in the household.

| MCP tool | Authenticated Axum operation | Input |
| --- | --- | --- |
| `household_users` | `GET /api/household-users` | None |
| `wishlists` | `GET /api/wishlists` | None |
| `create_wishlist` | `POST /api/wishlists` | `owner_id`, `name` |
| `add_wishlist_item` | `POST /api/wishlist/items` | `title`, `request_id`, optional `owner_id`, `list_id` |
| `wishlist_items` | `GET /api/wishlists/{list_id}/items` | `list_id` |

Every operation uses `Authorization: Bearer <API_KEY>`. Household Users and
Wishlists have stable IDs; lists return `id`, `owner_id`, `name`. A list name is
unique within its owner (SQLite NOCASE comparison), so different owners can each
have an `Ideas` list. Names allow 1–200 characters. Titles allow 1–500 characters.
Text is trimmed; empty values and control characters are rejected. This slice
stores generic titles only and rejects unrecognized body fields. It does not fetch
URLs, choose products, or expose rich item editing. The Dashboard also supports
minimal title editing through the same store.

## Dashboard management

Open Wishlist on the private Dashboard. All household members and their named
lists are visible together. Select a list inside its owner's group; additions,
title edits and removals carry that owner's stable ID and the explicit list ID.
Use **Manage household members** to add, rename or remove members. This is shared
household configuration, without passwords or an active-user selector.

| Authenticated Axum operation | Body / behavior |
| --- | --- |
| `PATCH /api/household-users/{owner_id}` | `{"name":"Member A"}`; preserve ID |
| `DELETE /api/household-users/{owner_id}` | Remove member, their lists and items |
| `PATCH /api/household-users/{owner_id}/wishlists/{list_id}` | `{"name":"Outdoors"}`; preserve owner and ID |
| `DELETE /api/household-users/{owner_id}/wishlists/{list_id}` | Remove this owner's list and all its items |
| `PATCH /api/household-users/{owner_id}/wishlists/{list_id}/items/{item_id}` | `{"title":"A warm jacket"}`; preserve destination and ID |
| `DELETE /api/household-users/{owner_id}/wishlists/{list_id}/items/{item_id}` | Remove only this owner's item in this list |

Successful edits return the updated resource; removal returns 204. Missing or
mismatched destinations return 404. Invalid text returns 400; duplicate names
return 409. Database cascades prevent orphaned items. The Dashboard explains the
scope of removal in a confirmation dialog, preserves failed drafts, and refreshes
from Axum after confirmed writes. Its credential stays on the Start server.
If a deleted list was the private `WISHLIST_DEFAULT_LIST_ID`, update that setting;
Axum continues to reject the missing default rather than guessing a replacement.

## Destination resolution

- An explicit `list_id` selects that list. A supplied `owner_id` must match its
  owner; a mismatch is a 409 conflict.
- An explicit `owner_id` without a list selects the owner's only list. Zero or
  multiple lists require clarification.
- With neither ID, Axum uses `WISHLIST_DEFAULT_LIST_ID` if configured privately.
  An invalid configured ID is a 404; it never falls back to a guessed list.
- Otherwise return 409 with `code: "wishlist_destination_required"`, one
  `question`, and `choices` containing list IDs, owner IDs and names. No item is
  written. The agent asks that question once using discovery to show owner names;
  it then supplies the selected IDs. With no lists, create a named list for the
  intended owner first.

Explicit owner/list inputs always override the private default. Household
identities, defaults and credentials belong in private runtime configuration.

## Retry contract

The agent generates a unique `request_id` (1–128 characters) for each intended
addition and retains it with the arguments until the outcome is known. A UUID is
suitable. It reuses the same ID and arguments on retries after transport failure;
it must not mint another ID just because a response was lost.

Axum atomically inserts the item with a globally unique request ID. The first
success returns 201 and `{"item":{"id","list_id","title","created_at"},
"replayed":false}`. An identical accepted retry returns 200 with the original
item's current value and `replayed:true`, even if the private default changed or the owner now has
another list. Identity compares the trimmed title and submitted owner/list IDs,
not a newly resolved destination. Reusing an accepted ID with different inputs
returns 409. A rejected request consumes no ID, so clarification can be followed
by a corrected request. A separate intentional addition uses a new ID, even when
its title is the same.

Deduplication persists in immutable `wishlist_addition_receipts` in the main
SQLite database. Migration 0007 backfills receipts for existing additions. The
receipt and new item commit together, so a failed addition consumes no request
ID. Title edits leave the original retry arguments unchanged; an accepted retry
reads the current title and never reverts an edit. Receipts survive item, list
and owner deletion: a delayed retry of a removed item returns 409 and never
recreates it. A new intentional addition needs a new request ID. Back up the main
database, including receipts.

The Dashboard keeps an unconfirmed addition's ID and arguments in tab-local
session storage before sending it. The same trimmed title/destination reuses that ID
across list navigation and page refresh. A confirmed result clears the pending
intention. If browser storage is unavailable, retries in the mounted form still
reuse the in-memory ID. After uncertain member/list creation, refresh discovery
before retrying; their names are unique within their defined scopes.

MCP marks reads read-only, creation non-destructive but non-idempotent, and
deduplicated additions non-destructive and idempotent. After uncertain list
creation, discover lists before retrying. API and transport failures are tool
errors (`isError:true`), never successful writes. All existing training tools
retain their read-only GET contracts.

## Acceptance evidence

The implementing agent can stand in for Hermes: an agent using the real OpenHome
MCP tools is the acceptance target, without a Hermes-specific executable or UI.
See the user clarification on [#73](https://github.com/patrickhaahr/openhome/issues/73#issuecomment-6113664694)
and [#97](https://github.com/patrickhaahr/openhome/issues/97#issuecomment-6113664863).

Automated proof runs through migrated SQLite and authenticated routes in
`api/tests/wishlist.rs`, and through the real MCP Streamable HTTP client → adapter
→ Axum in `mcp/tests/contract.rs`. Run `devenv shell -- just api-check mcp-check`.
For the agent workflow demonstration use an isolated migrated database and
synthetic owners: discover, create a named list, add a generic title, retry with
the same request ID, and read it back. Also demonstrate the clarification and
conflict outcomes; never publish real household names, runtime URLs or keys.
