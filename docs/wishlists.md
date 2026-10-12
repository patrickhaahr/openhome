# Shared Wishlists through the Dashboard and OpenHome MCP

Axum owns Household Users, named Wishlists, rich Wishlist Items and destination
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
| `wishlist_url_metadata` | `POST /api/wishlist/url-metadata` | Explicit `owner_id`, `list_id`, user-supplied `url` |
| `add_wishlist_item` | `POST /api/wishlist/items` | `title`, `request_id`, optional `owner_id`, `list_id`, rich fields |
| `wishlist_items` | `GET /api/wishlists/{list_id}/items` | `list_id` |
| `wishlist_item` | `GET /api/household-users/{owner_id}/wishlists/{list_id}/items/{item_id}` | `owner_id`, `list_id`, `item_id` |
| `update_wishlist_item` | `PATCH /api/household-users/{owner_id}/wishlists/{list_id}/items/{item_id}` | Explicit IDs, supplied item fields |
| `delete_wishlist_item` | `DELETE /api/household-users/{owner_id}/wishlists/{list_id}/items/{item_id}` | Explicit IDs |

Every operation uses `Authorization: Bearer <API_KEY>`. Household Users and
Wishlists have stable IDs; lists return `id`, `owner_id`, `name`. A list name is
unique within its owner (SQLite NOCASE comparison), so different owners can each
have an `Ideas` list. Names allow 1–200 characters. Titles allow 1–500 characters.
Text is trimmed; empty values and control characters are rejected, except line breaks
and tabs in notes. Unknown body fields are rejected. Generic requests remain generic:
no URL fetch, product choice or research runs implicitly. URL metadata assistance
is an explicit preview followed by a reviewed addition.

## Product URL assistance

Dashboard's **Read product details** and MCP's `wishlist_url_metadata` call the
same authenticated Axum operation with an explicit owner/list pair. Ownership is
checked before any outbound request. The operation never creates an item or a
mutation receipt. It returns `status` (`ready`, `partial`, `unavailable`), `url`,
and nullable `title`, `image_url`, `price` suggestions. Review and correct these
fields, then save with the existing add operation and a new `request_id`.

Open Graph/Twitter metadata and HTML title supply title/image suggestions;
Open Graph product price and currency supply price text. Relative image links
resolve against the fetched URL. The final fetched URL is returned; untrusted
canonical/`og:url` values are ignored. Oversized or invalid suggestions are
omitted. Scripts never run. Sites without these tags, or that require browser
execution or login, retain manual entry. Dashboard fills empty fields and keeps
existing text. Metadata is untrusted page content, never agent instructions.
Changing a product URL clears untouched generated fields; manually typed or
corrected values remain intact for review.

HTTP(S) URLs must have no credentials and a public destination. Every hop checks
all DNS answers and pins the vetted addresses for the actual connection, with
environment proxies and automatic redirects disabled. Loopback, local/private,
link-local, reserved, documentation and multicast address ranges are rejected,
including mapped IPv6 destinations. The request allows at most three redirects,
one MiB of uncompressed HTML, an eight-second overall network deadline and a
three-second connection deadline. Encoded and non-HTML responses are declined.
Failed extraction returns `unavailable` with the input URL; invalid or blocked
destinations are actionable API/tool errors. Neither result claims a saved wish.

After review, retain the **exact** add arguments and request ID for retries;
never refetch metadata to reconstruct an uncertain addition. Existing receipts
deduplicate URL additions, and changed arguments conflict. Preview retries can
return newer suggestions because they do not write anything.

## Item fields

| Field | API-owned contract |
| --- | --- |
| `title` | Required, 1–500 characters |
| `url`, `image_url` | Optional absolute HTTP(S) links, at most 2048 characters, without embedded credentials |
| `notes` | Optional text, 1–5000 characters; line breaks and tabs allowed |
| `price` | Optional text, 1–100 characters; include currency, e.g. `DKK 199.95`. No price arithmetic or assumed currency |
| `quantity` | Integer 1–999, defaults to 1 |
| `priority` | `low`, `normal` or `high`, defaults to `normal` |

All reads return these fields. Nullable fields default to null. Migration 0008
adds the defaults to existing items and immutable rich-field identity to addition
receipts. PATCH preserves omitted fields; explicit null clears URL/image/notes/price.
Title, quantity and priority cannot be null. Validation completes before the atomic
update so rejected edits never alter stored values. Every item edit/read/delete
checks the complete stable owner/list/item destination.

The dashboard's Item details panel supports these fields for both addition and
editing. Product and image links open only when selected, with no referrer; remote
images are not automatically fetched from a private household display.

## Dashboard management

Open Wishlist on the private Dashboard. All household members and their named
lists are visible together. Select a list inside its owner's group; additions,
item edits and removals carry that owner's stable ID and the explicit list ID.
Use **Manage household members** to add, rename or remove members. This is shared
household configuration, without passwords or an active-user selector.

| Authenticated Axum operation | Body / behavior |
| --- | --- |
| `PATCH /api/household-users/{owner_id}` | `{"name":"Member A"}`; preserve ID |
| `DELETE /api/household-users/{owner_id}` | Remove member, their wishlists, wishes and watchlist entries |
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
another list. Identity compares the trimmed title, normalized rich fields (including defaults) and submitted owner/list IDs,
not a newly resolved destination. Reusing an accepted ID with different inputs
returns 409. A rejected request consumes no ID, so clarification can be followed
by a corrected request. A separate intentional addition uses a new ID, even when
its title is the same.

Deduplication persists in immutable `wishlist_addition_receipts` in the main
SQLite database. Migration 0007 backfills receipts for existing additions. The
receipt and new item commit together, so a failed addition consumes no request
ID. Item edits leave the original retry arguments unchanged; an accepted retry
reads the current item and never reverts an edit. Receipts survive item, list
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
deduplicated additions non-destructive and idempotent. Item edits and deletes are
destructive and idempotent (reapplying the same values/removal cannot duplicate an
item). Successful deletion's API 204 maps to MCP `{ "deleted": true }`; a repeated
delete remains a not-found tool error. Read the item after an uncertain edit/delete
before retrying, so another household edit is not blindly overwritten. After uncertain list
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
