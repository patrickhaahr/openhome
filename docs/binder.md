# Binder: One Piece Card Game collections through the Dashboard and OpenHome MCP

Each Household User has one household-visible Binder of One Piece Card Game
cards. Terms are in [`docs/context/binder.md`](context/binder.md); the sources
and storage decisions are in [ADR 0017](adr/0017-binder-card-data-from-optcg.md).
[#287](https://github.com/patrickhaahr/openhome/issues/287) delivered the card
catalogue: Hermes looks a card up by Card Code and Axum stores it.
[#288](https://github.com/patrickhaahr/openhome/issues/288) adds Binder Entries:
Hermes adds cards from a photo and lists a Binder in Card Code order.
[#290](https://github.com/patrickhaahr/openhome/issues/290) corrects and removes
entries, from Hermes or from the Dashboard's card sheet. [#289](https://github.com/patrickhaahr/openhome/issues/289) adds the read-only
Dashboard Binder page: one owner's entries as spreads of two facing 3×3 pocket pages, with card images
relayed by the Dashboard's `/binder-image/{id}` route from the authenticated Axum
image read. Binder Value, values in other
currencies, the daily refresh and the rest of the Dashboard follow under epic [#285](https://github.com/patrickhaahr/openhome/issues/285).

## Operations

Every operation requires `Authorization: Bearer <API_KEY>`. MCP forwards one
authenticated Axum request per tool and returns its JSON unchanged.

| MCP tool | Authenticated Axum operation | Input |
| --- | --- | --- |
| `search_cards` | `GET /api/binder/cards?card_code=OP01-001` | A Card Code |
| none | `GET /api/binder/cards/{id}/image` | A Card `id`; returns the stored `image/avif` |
| `add_binder_entry` | `POST /api/binder/entries` | `card_id`, `owner_id`, `quantity`, `language`, `note`, `request_id` |
| `binder_entries` | `GET /api/binder/entries?owner_id=` | An optional owner filter |
| none | `GET /api/household-users/{owner_id}/binder/entries/{id}` | An explicit owner and entry |
| `update_binder_entry` | `PATCH /api/household-users/{owner_id}/binder/entries/{id}` | `owner_id`, `entry_id`, and any of `quantity`, `language`, `note` |
| `delete_binder_entry` | `DELETE /api/household-users/{owner_id}/binder/entries/{id}` | `owner_id`, `entry_id` |

## Card Codes and variants

A Card Code is a set prefix (one to four letters and at most two digits), a
hyphen and three digits: `OP01-001`, `ST01-012`, `EB01-001`, `P-001`. Axum trims
and upper-cases it, so `op01-001` works, and rejects anything else with 400.
Every code has one or more variants: the normal print and any parallel arts.
A Card's `id` is the catalogue's image identifier, kept opaque: `OP01-001`,
`OP04-016_P1`, `OP09-078-r1` and `EB03_OP09-034_p1` all occur. Axum checks only
that it is 1 to 64 letters, digits, hyphens or underscores starting with a letter
or digit, so it is safe in a URL path and the image route. The Card's
`card_code` is the variant's own Card Code field in the catalogue response, never
derived from the id, and a variant whose code differs from the one searched for
fails the lookup.

The search returns `{ "card_code": "OP01-001", "cards": [ ... ] }` with the
variants in catalogue order, normal print first. A code the catalogue does not
list returns `cards: []` with status 200. Each Card carries:

- the catalogue's details: `name`, `set_id`, `set_name`, `rarity`, `color`,
  `card_type`, `life`, `cost`, `power`, `counter_amount`, `attribute`,
  `sub_types`, `text` and `inventory_price_usd`. Numbers the catalogue sends as
  text (`life: "5"`) stay text, and an absent value is `null`.
- `card_value`: `{ "usd": 2.25, "observed_on": "2026-11-12" }`, the TCGplayer
  market price and the day the catalogue observed it, or `null` when the
  catalogue has no price. It is an object so that euro and krone amounts can join
  it when exchange rates exist, without changing what callers read today. The
  date can be months old; read it before treating the price as current.
- `image_path`: the authenticated read of the stored AVIF.

Axum fetches the `card_image` URL the catalogue gives for a variant, because
its filename can differ from the id (`OP09-058` serves `SPECIAL_BUGGY_BALL.jpg`).
It accepts the URL only when its scheme, host and port equal the configured
origin. A variant naming any other origin, or none, is an unavailable failure
and nothing is fetched from that origin. The URL is never stored.

## Binder Entries

A Binder Entry is one Card in one language in one owner's Binder: `id`, `owner_id`,
`card` (as search returns it, with its USD `card_value`), `language` (`en` or `ja`),
`quantity`, an optional `note` and `date_added`, the Europe/Copenhagen day it was
first added. There is no Binder record: a Binder is its owner's entries, and
removing the Household User removes them (the Cards stay). Each owner has at most
one entry per Card and language, so an English and a Japanese copy are two entries.
The catalogue's prices are for English printings, so a Japanese entry's Card has
neither `card_value` nor `inventory_price_usd`, in additions, lists and the stored
receipt result.

An addition names the Card by `card_id` and never sends card details. `quantity`
is a whole number of at least 1 (default 1) with no maximum, up to 4294967295 per
addition so that a sum of additions cannot overflow. `language` defaults to `en`,
and a note is 1 to 500 characters. Axum finds the Card in this order:

1. A stored Card (every variant a search returned) is used as it is.
2. Otherwise every substring of `card_id` that is a Card Code is a candidate, from
   left to right (`EB03_OP09-034_p1` contains `OP09-034`, while `EB03` is no code).
   Each candidate asks the catalogue (unless the code is already stored or known
   unlisted today), and the code's variants are stored only when the id is one of
   them, so a rejected addition stores no Card, image or unlisted marker. The
   spelling is only a hint that the catalogue's variant list verifies, so a wrong
   guess can only end in the next step.
3. Anything else, including an id that holds no code, a code that does not list it
   and a wrong-case id, is `404 card_not_listed` and writes nothing. The catalogue
   is queried only by Card Code, so an id holding no code can only be added after a
   search stored it; every variant id seen so far holds its code. Searching the
   Card Code first stays the recommended workflow.

A catalogue outage during the lookup is the existing `503 card_data_unavailable`.
Adding an existing owner, Card and language raises its quantity by the requested
amount and returns the updated entry; a supplied `note` replaces the stored one and
an omitted one keeps it. `date_added` keeps the first day.

A missing `owner_id` is `409 binder_destination_required` with a `question` and
`choices` (the Household Users), and writes nothing. An unknown owner is 404.

`request_id` is required (1 to 128 characters) and follows the Watchlist rules in
[`api-receipts.md`](api-receipts.md). The receipt is committed with the write and
stores the original response. An identical retry returns that result with
`replayed: true` and status 200 (a new addition is 201), so the quantity is not
added again and later merges do not change the replay. Different arguments under a
used id are 409. If the entry has since been removed, the retry is 409 and never
recreates it; the receipt outlives the entry and the owner. A rejected addition
rolls back with its claim and consumes no `request_id`.

`GET /api/binder/entries` returns every owner's entries, or one owner's with
`owner_id`, grouped by owner (name, then id). Each Binder is in Card Code order: set
prefix (compared alphabetically, so `EB` before `OP` before `P`, `PRB`, `ST`), then
set number, then card number, all numerically. Within a Card Code the normal print
(the variant whose id equals the code) comes first, then its parallels in the
catalogue's order, and an English entry precedes the Japanese one for the same Card.
Binder Value and Card Value in other currencies are not in the response yet.

### Correcting and removing

`PATCH` changes only the fields it names. An absent field keeps its stored value,
`note: null` clears the note, a supplied note is 1 to 500 characters, and
`quantity` replaces the stored count (it never adds to it) and is a whole number
from 1 to 4294967295. `quantity: null` and `language: null` are 400, as is an
unknown field such as `owner_id`; the update returns the stored entry, with a
Japanese entry's Card unpriced. `date_added` and the Card are never editable.

A language change to one the same owner already holds for that Card returns 409
and writes nothing, including its other fields: Axum never merges entries
silently, so correct the other entry's quantity and remove this one. Setting the
language an entry already has is not a collision.

The owner and the entry id are both part of the address. A wrong owner, an unknown
entry and an entry already removed are all 404 for read, update and delete, so a
repeated delete is not found. Deletion is final: it does not touch the Card, its
Price History or the accepted addition receipt, which stays retired so an old
`request_id` cannot recreate the entry.

Removing a Household User (`DELETE /api/household-users/{id}`) removes their Binder
Entries in the same statement as their Wishlists and Watchlist, through the
entries' foreign key. The Cards and their images stay. The Dashboard's removal
dialog says it deletes the person's wishlists, wishes, watchlist entries and Binder.

Migration 0032 adds `binder_entries` and `binder_addition_receipts`. Rolling it back
discards every entry and receipt.

## Storing and the catalogue

A search asks the catalogue only when nothing is stored for the code. It then
fetches every variant's image, re-encodes it with the shared AVIF encoder
(speed 8, quality 70, never scaled up) and writes all variants and images in one
transaction. Later searches and image reads are answered from OpenHome and make
no catalogue request. Stored Cards are snapshots that clients never edit.
A code is unlisted only when the catalogue answers 404 with its own
`{"error":"Card not found."}` body; any other 404 is a `card_data_unavailable`
failure. An unlisted code is remembered in SQLite until the next Europe/Copenhagen
calendar day: a repeat that day makes no catalogue request and returns
`cards: []`, and a later day asks again, so a card the catalogue lists later can
still be found.

The catalogue is one volunteer's server. The adapter uses the fixed
`https://optcgapi.com/` origin (replaceable only in code, for tests), a five
second timeout, a 256 KiB limit for card JSON and 2 MiB for an image, no
redirects, no retries and the app's `User-Agent`. Unreachable, slow, refused,
oversized or malformed answers, a variant of another code, a price without its
date, and an image that is not a PNG or JPEG are all a sanitised 503 with
`code: "card_data_unavailable"`. Upstream text only reaches the log. A failure
writes nothing, so the next search asks again. Hermes should tell the user to try
later.

Migration 0031 adds the `cards` and `unlisted_cards` tables. Rolling it back
discards every stored Card, image and unlisted-code marker.

## Verification

Run `devenv shell -- just api-check mcp-check`. The domain proof is
`api/tests/cards.rs`: authenticated HTTP on migrated SQLite against a loopback
catalogue (first lookup, cached lookup, unlisted and malformed codes, each
failure kind, image serving). `api/tests/binder.rs` covers additions, owner
clarification, merging, unlisted cards, outages and ordering,
`api/tests/binder_retries.rs` covers receipts and simultaneous retries, and the
Card Code ordering unit test is in `api/src/services/binder.rs`.
`api/tests/binder_edits.rs` covers partial updates, validation, the language
collision, wrong owners, deletion and the Household User cascade.
`mcp/tests/binder.rs` drives `search_cards`, `add_binder_entry`, `binder_entries`,
`update_binder_entry` and `delete_binder_entry` through the real Streamable HTTP
adapter. `dashboard/tests/binder.test.tsx` renders the card sheet: its details,
edits, removal with confirmation and failure recovery. For a running stack, follow the
verify-openhome Binder recipe
([`features/binder.md`](../.agents/skills/verify-openhome/features/binder.md)): the
synthetic preview serves invented `OP99` cards from a loopback catalogue and logs
every request it receives. Nothing here contacts the live catalogue; its shape
was read once by hand from the public docs and one sample request.
