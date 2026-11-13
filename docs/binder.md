# Binder: One Piece Card Game collections through the Dashboard and OpenHome MCP

Each Household User will have one household-visible Binder of One Piece Card
Game cards. Terms are in [`docs/context/binder.md`](context/binder.md); the
sources and storage decisions are in [ADR 0017](adr/0017-binder-card-data-from-optcg.md).
This slice, [#287](https://github.com/patrickhaahr/openhome/issues/287), delivers
the card catalogue: Hermes looks a card up by Card Code and Axum stores it. Binder
Entries, values in other currencies, the daily refresh and the Dashboard follow
under epic [#285](https://github.com/patrickhaahr/openhome/issues/285).

## Operations

Every operation requires `Authorization: Bearer <API_KEY>`. MCP forwards one
authenticated Axum request per tool and returns its JSON unchanged.

| MCP tool | Authenticated Axum operation | Input |
| --- | --- | --- |
| `search_cards` | `GET /api/binder/cards?card_code=OP01-001` | A Card Code |
| none | `GET /api/binder/cards/{id}/image` | A Card `id`; returns the stored `image/avif` |

## Card Codes and variants

A Card Code is a set prefix (one to four letters and at most two digits), a
hyphen and three digits: `OP01-001`, `ST01-012`, `EB01-001`, `P-001`. Axum trims
and upper-cases it, so `op01-001` works, and rejects anything else with 400.
Every code has one or more variants: the normal print and any parallel arts.
A Card's `id` is the catalogue's image identifier, the code plus an optional
lower-case suffix (`OP01-001`, `OP01-001_p1`).

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

Image URLs from the catalogue are never stored or followed. Axum builds each
image address from its own configured origin and the validated variant id.

## Storing and the catalogue

A search asks the catalogue only when nothing is stored for the code. It then
fetches every variant's image, re-encodes it with the shared AVIF encoder
(speed 8, quality 70, never scaled up) and writes all variants and images in one
transaction. Later searches and image reads are answered from OpenHome and make
no catalogue request. Stored Cards are snapshots that clients never edit.
An unlisted code is not remembered, so it asks the catalogue again.

The catalogue is one volunteer's server. The adapter uses the fixed
`https://optcgapi.com/` origin (replaceable only in code, for tests), a five
second timeout, a 256 KiB limit for card JSON and 2 MiB for an image, no
redirects, no retries and the app's `User-Agent`. Unreachable, slow, refused,
oversized or malformed answers, a variant of another code, a price without its
date, and an image that is not a PNG or JPEG are all a sanitised 503 with
`code: "card_data_unavailable"`. Upstream text only reaches the log. A failure
writes nothing, so the next search asks again. Hermes should tell the user to try
later.

Migration 0031 adds the `cards` table. Rolling it back discards every stored
Card and image.

## Verification

Run `devenv shell -- just api-check mcp-check`. The domain proof is
`api/tests/cards.rs`: authenticated HTTP on migrated SQLite against a loopback
catalogue (first lookup, cached lookup, unlisted and malformed codes, each
failure kind, image serving). `mcp/tests/binder.rs` drives `search_cards`
through the real Streamable HTTP adapter. For a running stack, follow the
verify-openhome Binder recipe
([`features/binder.md`](../.agents/skills/verify-openhome/features/binder.md)): the
synthetic preview serves invented `OP99` cards from a loopback catalogue and logs
every request it receives. Nothing here contacts the live catalogue; its shape
was read once by hand from the public docs and one sample request.
