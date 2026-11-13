# Binder

Hermes looks up a One Piece Card Game card by its Card Code and gets every variant with its details, USD Card Price, the price's observation date and a stored AVIF image. Hermes then adds cards to a Household User's Binder from a photo and lists the Binder in Card Code order. Only the catalogue and Binder Entry slices exist so far: there is no editing or removal of entries, no currency other than USD, no Binder Value and no Dashboard view yet.

## Sub-features

- `binder-search-mcp` calls `search_cards` for an invented code and gets the normal print and its parallel.
- `binder-cached` repeats the lookup and the catalogue stand-in logs no further request.
- `binder-unlisted` searches a code the catalogue does not list and gets an empty `cards` list; repeating it the same day makes no second catalogue request.
- `binder-odd-ids` searches `OP99-020`, whose special print has the id `EB99_OP99-020_P1` and an image file named `SPECIAL_LANTERN_SPIRIT.jpg`, and reads its image.
- `binder-malformed` sends a malformed code and gets a 400 tool error.
- `binder-outage` searches the outage code and gets `card_data_unavailable` (503) with no upstream text.
- `binder-image` reads a stored image through authenticated Axum and gets `image/avif`.
- `binder-add-no-owner` calls `add_binder_entry` without an owner and gets `binder_destination_required` with choices; nothing is written.
- `binder-add` adds a card for an explicit owner and reads it back.
- `binder-retry` repeats the identical call: `replayed: true`, quantity unchanged. Reusing the `request_id` with a changed quantity is a 409.
- `binder-merge` adds the same card again (a new `request_id`): the quantity rises on the same entry.
- `binder-ja` adds a `ja` copy: a separate entry.
- `binder-unlisted-add` adds a card the catalogue does not list: `card_not_listed`, nothing written.
- `binder-list` lists one owner and every owner, in Card Code order with a parallel after its normal print.

## How to get to it (user POV)

- Ask Hermes to look up a card by the code printed on it (MCP tool `search_cards`).
- Ask Hermes to add the cards in a photo (MCP tools `add_binder_entry`, `binder_entries`).
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
- **Owners.** `mcp.sh call household_users '{}'` gives the seeded Household Users (Alex, Robin, Sam); IDs change per launch.
- **No owner.** Search first (`search_cards` for `OP99-001`), then `mcp.sh call add_binder_entry '{"card_id":"OP99-001","request_id":"verify-b1"}'` exits 2 with `code: "binder_destination_required"`, a `question` and one choice per Household User. `binder_entries` with no filter is still `[]`.
- **Add.** Repeat with `"owner_id":"<Alex id>"`. `structuredContent` has `replayed: false` and an `entry` with `quantity` 1, `language` `en`, `date_added` today in Copenhagen and the full `card`. Read it back with `binder_entries` `{"owner_id":"<Alex id>"}` and `curl` of `$API_URL/api/binder/entries?owner_id=<id>`: the same entry.
- **Retry.** Call the identical arguments again. `replayed: true`, the same `entry`, and the list still shows quantity 1. Now reuse `verify-b1` with `"quantity":2`: exit 2, status 409, nothing changed.
- **Merge.** A new `request_id` with `"quantity":2` for the same card: the same entry `id`, `quantity` 3. Retry the first call again: it still replays the original quantity 1, while the list shows 3.
- **Japanese.** Add the same card with `"language":"ja"`: a second entry with its own `id`, quantity 1.
- **Unlisted.** `add_binder_entry` with `"card_id":"OP99-777"` exits 2 with `status: 404`, `code: "card_not_listed"`. The list is unchanged and the stand-in log shows one 404 for it (or none if the code was already searched unlisted today). A parallel id that was never searched, such as `OP99-002_p1`, is rejected the same way.
- **Order.** Search `OP99-002`, then add `OP99-010`, the parallel `OP99-002_p1`, `OP99-002` and `OP99-001_p1` for Alex, and one card for Robin. `binder_entries` with no filter lists Alex's Binder (the earlier entries included) in Card Code order, `OP99-001` en then ja, `OP99-001_p1`, `OP99-002`, `OP99-002_p1`, `OP99-010`, followed by Robin's.
- **Proof.** Save each tool JSON as `01-…` onward, plus the stand-in log lines, under `$EVIDENCE_DIR/binder/`.

## Gotchas

- Add by the Card `id` from `search_cards`. A parallel that was never searched cannot be added, because Axum only tries an id as a Card Code for a normal print.
- Card IDs are opaque catalogue image identifiers (`OP99-001_p1`, `EB99_OP99-020_P1`), stable across launches, unlike Household User IDs. A card's image file name can differ from its id.
- The synthetic images are flat colour drawings, so their AVIF sizes say nothing about real cards.
- The stand-in sits behind a loopback port that changes per launch; read it from the `OPTCG: loopback catalogue at …` line in the stack log if you need it.
