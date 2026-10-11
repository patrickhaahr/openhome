# Generic wishes through the OpenHome MCP

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
URLs, choose products, or expose rich item editing.

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
item and `replayed:true`, even if the private default changed or the owner now has
another list. Identity compares the trimmed title and submitted owner/list IDs,
not a newly resolved destination. Reusing an accepted ID with different inputs
returns 409. A rejected request consumes no ID, so clarification can be followed
by a corrected request. A separate intentional addition uses a new ID, even when
its title is the same.

Deduplication persists with the item in the main SQLite database. This slice has
no deletion/edit tools; the migration cascades item deletion with its list and
list deletion with its owner. Future editing/deletion must preserve the retry
contract deliberately. Back up the main database, including these tables.

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
