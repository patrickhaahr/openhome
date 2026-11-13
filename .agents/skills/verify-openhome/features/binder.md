# Binder

Hermes looks up a One Piece Card Game card by its Card Code and gets every variant with its details, USD Card Price, the price's observation date and a stored AVIF image. Only this catalogue slice exists so far: there are no Binder Entries, currencies other than USD, or Dashboard view yet.

## Sub-features

- `binder-search-mcp` calls `search_cards` for an invented code and gets the normal print and its parallel.
- `binder-cached` repeats the lookup and the catalogue stand-in logs no further request.
- `binder-unlisted` searches a code the catalogue does not list and gets an empty `cards` list; repeating it the same day makes no second catalogue request.
- `binder-odd-ids` searches `OP99-020`, whose special print has the id `EB99_OP99-020_P1` and an image file named `SPECIAL_LANTERN_SPIRIT.jpg`, and reads its image.
- `binder-malformed` sends a malformed code and gets a 400 tool error.
- `binder-outage` searches the outage code and gets `card_data_unavailable` (503) with no upstream text.
- `binder-image` reads a stored image through authenticated Axum and gets `image/avif`.

## How to get to it (user POV)

- Ask Hermes to look up a card by the code printed on it (MCP tool `search_cards`).
- No Dashboard route yet. Images are read from `GET /api/binder/cards/{id}/image` with the bearer key.

## Driving it with verify-openhome

Preconditions:

- `doctor.sh` prints `OK`.
- The loopback catalogue knows only invented `OP99` cards: `OP99-001` and `OP99-002` have a parallel, `OP99-010` has one variant, `OP99-020` has two variants with odd spellings (id `EB99_OP99-020_P1`, image file `SPECIAL_LANTERN_SPIRIT.jpg`). `OP99-503` makes the stand-in answer 503. Any other code is unlisted. Never use a real code such as `OP01-001`: the preview must not reach the live catalogue, and the stand-in would answer 404 anyway.
- Fresh launch, so nothing is stored. Data resets on relaunch.

- **First lookup.** Run `mcp.sh call search_cards '{"card_code":"OP99-001"}'`. `structuredContent.cards` has ids `OP99-001` and `OP99-001_p1`, each with `card_value` (`usd`, `observed_on`) and `image_path`. The parallel's price is about five months old.
- **Count the requests.** The stand-in logs every request to the stack log as `OPTCG stand-in: GET <path> -> <status>`. Run `grep -c "OPTCG stand-in" "$RUN_DIR/logs/stack.log"`: the first lookup adds three lines (the card and two images).
- **Cached.** Repeat the call (lower case `op99-001` works too). The result is identical and the count is unchanged.
- **Unlisted.** `mcp.sh call search_cards '{"card_code":"OP99-777"}'` returns `{"card_code":"OP99-777","cards":[]}`. The log shows one 404 line. Repeat it: the result is the same and the log gains no line, because Axum remembers the unlisted code until the next Copenhagen day (a later day asks again, which this recipe cannot show without changing the clock).
- **Odd ids.** `mcp.sh call search_cards '{"card_code":"OP99-020"}'` returns ids `OP99-020` and `EB99_OP99-020_P1`, both with `card_code` `OP99-020`. The log shows the special print's image fetched as `/media/static/Card_Images/SPECIAL_LANTERN_SPIRIT.jpg -> 200`, not `EB99_OP99-020_P1.jpg`. Reading `/api/binder/cards/EB99_OP99-020_P1/image` gives `image/avif`.
- **Malformed.** `mcp.sh call search_cards '{"card_code":"OP99"}'` exits 2 with `status: 400` and no log line.
- **Outage.** `mcp.sh call search_cards '{"card_code":"OP99-503"}'` exits 2 with `code: "card_data_unavailable"`, `status: 503`. A repeat asks the stand-in again (nothing was stored).
- **Image.** `curl -sS -D - -o /dev/null -H "Authorization: Bearer $API_KEY" "$API_URL/api/binder/cards/OP99-001_p1/image"` shows `content-type: image/avif`. Without the key it is 401, for an unknown well-formed id such as `OP99-999_p1` 404, and for a malformed id 400.
- **Proof.** Save each tool JSON as `01-…` onward, plus the stand-in log lines, under `$EVIDENCE_DIR/binder/`.

## Gotchas

- Card IDs are opaque catalogue image identifiers (`OP99-001_p1`, `EB99_OP99-020_P1`), stable across launches, unlike Household User IDs. A card's image file name can differ from its id.
- The synthetic images are flat colour drawings, so their AVIF sizes say nothing about real cards.
- The stand-in sits behind a loopback port that changes per launch; read it from the `OPTCG: loopback catalogue at …` line in the stack log if you need it.
