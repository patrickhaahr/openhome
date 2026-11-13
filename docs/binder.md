# Binder: One Piece Card Game collections through the Dashboard and OpenHome MCP

Each Household User has one household-visible Binder of One Piece Card Game
cards. Terms are in [`docs/context/binder.md`](context/binder.md); the sources
and storage decisions are in [ADR 0017](adr/0017-binder-card-data-from-optcg.md).
[#287](https://github.com/patrickhaahr/openhome/issues/287) delivered the card
catalogue: Hermes looks a card up by Card Code and Axum stores it.
[#288](https://github.com/patrickhaahr/openhome/issues/288) adds Binder Entries:
Hermes adds cards from a photo and lists a Binder in Card Code order. Editing and
removing entries, Binder Value, values in other currencies, the daily refresh and
the Dashboard follow under epic [#285](https://github.com/patrickhaahr/openhome/issues/285).

## Operations

Every operation requires `Authorization: Bearer <API_KEY>`. MCP forwards one
authenticated Axum request per tool and returns its JSON unchanged.

| MCP tool | Authenticated Axum operation | Input |
| --- | --- | --- |
| `search_cards` | `GET /api/binder/cards?card_code=OP01-001` | A Card Code |
| none | `GET /api/binder/cards/{id}/image` | A Card `id`; returns the stored `image/avif` |
| `add_binder_entry` | `POST /api/binder/entries` | `card_id`, `owner_id`, `quantity`, `language`, `note`, `request_id` |
| `binder_entries` | `GET /api/binder/entries?owner_id=` | An optional owner filter |

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
no `card_value`.

An addition names the Card by `card_id` and never sends card details. `quantity`
is 1 to 999 (default 1), and a merge that would take an entry past 999 copies is
rejected with 400 and changes nothing. `language` defaults to `en`, and a note is
1 to 500 characters. Axum finds the Card in this order, and never reads a code out of an
id's spelling:

1. A stored Card (every variant a search returned) is used as it is.
2. Otherwise `card_id` is tried as a Card Code, which is the id of a normal print.
   Axum looks that code up through the search path and accepts the id only if it is
   one of the code's variants.
3. Anything else, including a parallel that was never searched for, an unlisted
   code and a wrong-case id, is `404 card_not_listed` and writes nothing. Hermes
   searches the Card Code first, so its ids are stored by the time it adds them.

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
`mcp/tests/binder.rs` drives `search_cards`, `add_binder_entry` and
`binder_entries` through the real Streamable HTTP adapter. For a running stack, follow the
verify-openhome Binder recipe
([`features/binder.md`](../.agents/skills/verify-openhome/features/binder.md)): the
synthetic preview serves invented `OP99` cards from a loopback catalogue and logs
every request it receives. Nothing here contacts the live catalogue; its shape
was read once by hand from the public docs and one sample request.
