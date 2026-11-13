# Binder

Hermes looks up a One Piece Card Game card by its Card Code and gets every variant with its details, its Card Value in DKK, EUR and USD with the price's observation date and a stale flag, and a stored AVIF image. Hermes then adds cards to a Household User's Binder from a photo and lists the Binder in Card Code order. The Dashboard shows each Binder as spreads of two facing 3×3 pocket pages. Tapping a pocket opens a card sheet that edits quantity, language and note, or removes the entry after confirmation. Hermes does the same with `update_binder_entry` and `delete_binder_entry`. Binder reads carry each owner's Binder Value, and the Dashboard shows it in DKK, EUR or USD by a switch kept in a cookie.

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
- `binder-add-unsearched` adds the parallel `OP99-002_p1` before anyone searched `OP99-002`: the Card is stored and the entry listed.
- `binder-ja` adds a `ja` copy: a separate entry with no `card_value` and `value_unavailable: "japanese_printing"`.
- `binder-unlisted-add` adds a card the catalogue does not list: `card_not_listed`, nothing written.
- `binder-list` lists one owner and every owner, in Card Code order with a parallel after its normal print.
- `binder-dash-open` opens Binder from Main navigation on the Dashboard (Home has the link, with no summary) and shows Alex's first spread.
- `binder-dash-page` turns whole spreads by the arrow buttons and the ArrowLeft/ArrowRight keys, in landscape and portrait.
- `binder-dash-owner` switches between Alex, Robin and Sam under `Whose Binder`.
- `binder-dash-empty` opens Sam, whose Binder is empty.
- `binder-dash-images` shows that every card image comes from the Dashboard origin.
- `binder-dash-failed` shows the failed read with its recovery key, and recovery.
- `binder-edit-mcp` calls `update_binder_entry` for quantity, language and note, and reads each result back.
- `binder-edit-clear` sends `"note":null` and the note is cleared; omitted fields stay.
- `binder-edit-invalid` sends quantity 0, a null quantity and a blank note and gets 400 each time with nothing changed.
- `binder-edit-collision` moves an entry to a language another entry of the owner and Card holds: 409 and both entries unchanged.
- `binder-edit-404` uses another owner's entry id, or an unknown one: 404.
- `binder-delete-mcp` calls `delete_binder_entry`: `{"deleted":true}`, the entry is gone from the list; a repeat is 404.
- `binder-dash-sheet` taps a pocket and sees the large image, details, quantity, language and note, in landscape and portrait.
- `binder-dash-edit` saves a change in the sheet and sees the stored result read back.
- `binder-dash-edit-fail` makes the save collide (409): the sheet stays open with an error and the draft.
- `binder-dash-remove` removes an entry after confirming; `Keep it` keeps it.
- `binder-value-mcp` reads `card_value` in three currencies, `stale` flags, an unpriced `ja` entry and `binder_values` through MCP.
- `binder-value-dash` shows the Binder Value in the header and the Card Value with its date and Stale badge in the card sheet.
- `binder-value-currency` switches DKK, EUR and USD; the choice survives a reload, and the initial HTML already holds it.
- `binder-value-outage` starts with `OPENHOME_PREVIEW_ECB=outage`: today's rates never arrive and the earlier ones stay in use.
- `binder-household-remove` removes a Household User from `Household and lists`: the dialog names the Binder, their entries go and the cards stay.

## How to get to it (user POV)

- Ask Hermes to look up a card by the code printed on it (MCP tool `search_cards`).
- Ask Hermes to add the cards in a photo (MCP tools `add_binder_entry`, `binder_entries`).
- Open Binder in the Dashboard's `Main navigation` (`/binder`). Pick whose Binder under `Whose Binder`; Everyone is not offered, since a Binder belongs to one person.
- Images are read from `GET /api/binder/cards/{id}/image` with the bearer key, and by the browser only from the Dashboard's `/binder-image/{id}`.

## Driving it with verify-openhome

Preconditions:

- `doctor.sh` prints `OK`.
- The loopback catalogue knows only invented `OP99` cards: `OP99-001`, `OP99-002` and `OP99-004` have a parallel, `OP99-003`, `OP99-005`, `OP99-006`, `OP99-010` and `OP99-011` have one variant, `OP99-020` has two variants with odd spellings (id `EB99_OP99-020_P1`, image file `SPECIAL_LANTERN_SPIRIT.jpg`). `OP99-503` makes the stand-in answer 503. Any other code is unlisted. Never use a real code such as `OP01-001`: the preview must not reach the live catalogue, and the stand-in would answer 404 anyway.
- The ECB stand-in serves the file at its production path from a loopback port, logged as `ECB stand-in: GET <path> -> <status>` (the port is on the `ECB: loopback reference rates at <host:port>` line). The preview stores invented rates for every working day of the last 200 days, and the startup refresh stores today's. Start with `OPENHOME_PREVIEW_ECB=outage` in the environment of `up.sh` for the outage step. Never contact the live bank.
- Fresh launch. The preview seeds Binder Entries through the production route at startup (`seed/binder.json`), using only `OP99-003` to `OP99-006` and `OP99-011` to `OP99-015`: Alex has 12 entries (a `ja` entry, quantities 2 and 3), Robin 4, Sam none. It leaves `OP99-001`, `OP99-002`, `OP99-010` and `OP99-020` alone, so the card steps below still start from nothing stored, but every owner step uses Sam, whose Binder is empty. Run the Dashboard steps that need Sam empty (`binder-dash-empty`) before the Hermes add steps. Data resets on relaunch.

- **First lookup.** Run `mcp.sh call search_cards '{"card_code":"OP99-001"}'`. `structuredContent.cards` has ids `OP99-001` and `OP99-001_p1`, each with `card_value` (`usd`, `eur`, `dkk`, `observed_on`, `rate_date`, `stale`) and `image_path`. The parallel's price is about five months old.
- **Count the requests.** The stand-in logs every request to the stack log as `OPTCG stand-in: GET <path> -> <status>`. Run `grep -c "OPTCG stand-in" "$RUN_DIR/logs/stack.log"`: the first lookup adds three lines (the card and two images).
- **Cached.** Repeat the call (lower case `op99-001` works too). The result is identical and the count is unchanged.
- **Unlisted.** `mcp.sh call search_cards '{"card_code":"OP99-777"}'` returns `{"card_code":"OP99-777","cards":[]}`. The log shows one 404 line. Repeat it: the result is the same and the log gains no line, because Axum remembers the unlisted code until the next Copenhagen day (a later day asks again, which this recipe cannot show without changing the clock).
- **Odd ids.** `mcp.sh call search_cards '{"card_code":"OP99-020"}'` returns ids `OP99-020` and `EB99_OP99-020_P1`, both with `card_code` `OP99-020`. The log shows the special print's image fetched as `/media/static/Card_Images/SPECIAL_LANTERN_SPIRIT.jpg -> 200`, not `EB99_OP99-020_P1.jpg`. Reading `/api/binder/cards/EB99_OP99-020_P1/image` gives `image/avif`.
- **Malformed.** `mcp.sh call search_cards '{"card_code":"OP99"}'` exits 2 with `status: 400` and no log line.
- **Outage.** `mcp.sh call search_cards '{"card_code":"OP99-503"}'` exits 2 with `code: "card_data_unavailable"`, `status: 503`. A repeat asks the stand-in again (nothing was stored).
- **Image.** `curl -sS -D - -o /dev/null -H "Authorization: Bearer $API_KEY" "$API_URL/api/binder/cards/OP99-001_p1/image"` shows `content-type: image/avif`. Without the key it is 401, for an unknown well-formed id such as `OP99-999_p1` 404, and for a malformed id 400.
- **Owners.** `mcp.sh call household_users '{}'` gives the seeded Household Users (Alex, Robin, Sam); IDs change per launch.
- **No owner.** `mcp.sh call add_binder_entry '{"card_id":"OP99-001","request_id":"verify-b1"}'` exits 2 with `code: "binder_destination_required"`, a `question` and one choice per Household User. `binder_entries` with Sam's `owner_id` is still `[]`.
- **Add.** Repeat with `"owner_id":"<Sam id>"`. `structuredContent` has `replayed: false` and an `entry` with `quantity` 1, `language` `en`, `date_added` today in Copenhagen and the full `card`. Read it back with `binder_entries` `{"owner_id":"<Sam id>"}` and `curl` of `$API_URL/api/binder/entries?owner_id=<id>`: the same entry.
- **Retry.** Call the identical arguments again. `replayed: true`, the same `entry`, and the list still shows quantity 1. Now reuse `verify-b1` with `"quantity":2`: exit 2, status 409, nothing changed.
- **Merge.** A new `request_id` with `"quantity":2` for the same card: the same entry `id`, `quantity` 3. Retry the first call again: it still replays the original quantity 1, while the list shows 3.
- **Japanese.** Add the same card with `"language":"ja"`: a second entry with its own `id`, quantity 1, and `card.card_value` `null` with `card.value_unavailable` `japanese_printing` (the `en` entry keeps its `card_value`), also in `binder_entries`.
- **Unsearched parallel.** On a fresh launch, `add_binder_entry` with `"card_id":"OP99-002_p1"` for Sam succeeds without a prior `search_cards`: the stand-in log shows the `OP99-002` lookup and its images, and `binder_entries` lists the entry. A later `search_cards` for `OP99-002` adds no log line.
- **Unlisted.** `add_binder_entry` with `"card_id":"OP99-777"` exits 2 with `status: 404`, `code: "card_not_listed"`. The list is unchanged and the stand-in log shows one 404 for it (or none if the code was already searched unlisted today). An id that holds no listed code, such as `OP99-002_p9` (the code lists no such variant), is rejected the same way.
- **Order.** Search `OP99-002`, then add `OP99-010`, the parallel `OP99-002_p1`, `OP99-002` and `OP99-001_p1` for Sam, and one card for Robin. `binder_entries` with Sam's `owner_id` lists the earlier entries too, in Card Code order, `OP99-001` en then ja, `OP99-001_p1`, `OP99-002`, `OP99-002_p1`, `OP99-010`.
- **Dashboard open.** Open `$DASHBOARD_URL`'s port in a T3 preview tab, then apply the `ipad-air` preset (landscape) a second time after opening: the preview ignores the first. Click `Binder` in `Main navigation`. Alex's first spread shows two facing pages (`Page 1` with nine pockets, `Page 2` with three filled and six empty) and `Pages 1–2 of 2`; the credit line names the OPTCG catalogue. Open the tab with `open: true`: a background tab leaves the pictures undecoded, so only their code and name show.
- **Dashboard paging.** The seed leaves Alex one spread. To see the second, add eight entries for Alex with `add_binder_entry` (`OP99-001` en and ja, `OP99-001_p1`, `OP99-002`, `OP99-002_p1`, `OP99-010`, `OP99-020`, `EB99_OP99-020_P1`), which makes 20: the first spread is full (the right page starts at the 10th entry, `OP99-005`), the second shows two entries on `Page 3`, an empty `Page 4` and `Pages 3–4 of 4`. Click `Next pages`: `Next pages` is disabled. Press `ArrowLeft` to return. The URL carries `person` and `page` (the spread number). Repeat with `preview_resize` `orientation: portrait`: both pages stay side by side with smaller pockets and the page does not scroll horizontally.
- **Dashboard markers.** Alex's `OP99-012` pocket shows `×3`, `OP99-005` `×2` and the `OP99-004` Japanese entry `JP` . A quantity of 1 shows nothing.
- **Dashboard owners.** Click `Robin`: four pockets (one `JP`, one `×4`), `Pages 1–2 of 2`. Click `Sam`: "Sam’s Binder is empty" and no page controls.
- **Dashboard images.** In the tab, `preview_evaluate` the `.pocket img` `src` values: all `/binder-image/<id>` on the Dashboard origin, and `performance.getEntriesByType('resource')` holds no entry outside it. `curl "$DASHBOARD_URL/binder-image/OP99-001"` gives `image/avif`, an unknown or malformed id 404. The stack log's catalogue lines do not grow when the Dashboard loads.
- **Dashboard failed read.** The stack cannot lose its API without taking the Dashboard down (`dashboard-synthetic` stops both). Start a second `vite dev` on a free port with `OPENHOME_API_URL` pointing at a port nothing listens on and `API_KEY=synthetic-dashboard-api-key`, open `/binder` there: "Binder unavailable" and a `Check again` button, no empty Binder. Then start any GET forwarder on that port to `$API_URL` (with the bearer header) and press `Check again`: the pockets appear. Stop both afterwards.
- **Edit.** Take Sam's entry ids from `binder_entries`. `mcp.sh call update_binder_entry '{"owner_id":"<Sam>","entry_id":"<id>","quantity":4}'` returns the entry with quantity 4 and the same language and note. Add `"note":"sleeve"`, then `"note":null` (cleared), then `"language":"ja"`: each result equals the `binder_entries` read-back.
- **Edit invalid.** `"quantity":0`, `"quantity":null`, `"note":"  "` and an unknown field each exit 2 with status 400. The entry is unchanged in the list.
- **Edit collision.** With Sam holding the same Card in `en` and `ja`, update one to the other's language: exit 2, status 409, both entries unchanged in the list.
- **Edit 404.** Another owner's `owner_id` with Sam's `entry_id`, or a random uuid: exit 2, status 404.
- **Delete.** `mcp.sh call delete_binder_entry '{"owner_id":"<Sam>","entry_id":"<id>"}'` gives `{"deleted":true}` and the entry leaves `binder_entries`. Repeat it: exit 2, status 404. The Card stays: `/api/binder/cards/<id>/image` is still 200.
- **Dashboard sheet.** Open `/binder?person=<Sam id>` (or pick Sam) at the `ipad-air` preset and click a pocket. The sheet shows the large image, owner, code, `Added` date, Set, Rarity, Colour, Type, Cost, Power, Counter, Attribute, Types and card text, then Quantity, Language, Note, `Save changes` and `Remove`. Repeat in portrait: it still fits.
- **Dashboard edit.** Change the quantity and press `Save changes`: the pocket badge shows the new quantity and `curl "$API_URL/api/binder/entries?owner_id=<id>"` agrees. The sheet shows the stored values.
- **Dashboard edit failure.** With Sam holding the Card in `en` and `ja`, switch one to the other language and save: the sheet stays open with the conflict message and your draft, and the list is unchanged. Choose the original language and save again to recover.
- **Dashboard remove.** Press `Remove`: a confirmation names the card, its language and the owner. `Keep it` returns to the form with the entry intact. `Remove for good` closes the sheet and the pocket disappears (the API read-back agrees); the card image stays.
- **Household removal.** Create a throwaway user in `Household and lists` on `/lists?tab=wish`, add entries for them with `add_binder_entry`, then `Remove <name>`: the confirmation reads "with all their wishlists, wishes and watchlist entries, and their Binder". Note one of their entry ids first. After `Remove for good`, `GET $API_URL/api/household-users/<id>/binder/entries/<entry id>` is 404, `/api/binder/entries?owner_id=<id>` is `[]`, other owners are unchanged and the card images are still 200. Use a throwaway user: the seeded ones are shared by later steps. In the preview, the buttons read `Remove <name>` through their text, not an `aria-label`; click them with `preview_evaluate` since selectors with quotes are rejected.
- **Values.** `mcp.sh call binder_entries '{"owner_id":"<Alex>"}'` returns `entries` and `binder_values` (one item for Alex: `usd`, `eur`, `dkk`, `unpriced_entries`, `oldest_observed_on`). Alex's `OP99-006` price is 45 days old and `OP99-015` 31 days old: `stale: true`, and `OP99-012` is fresh: `stale: false`. Alex's `ja` entries have `card_value: null` and `value_unavailable: "japanese_printing"`, and count in `unpriced_entries`. Each priced entry's `rate_date` is the working day on or before its `observed_on`. The `dkk`, `eur` and `usd` totals equal quantity times each card's value summed over the priced entries, including stale ones, to the cent.
- **Dashboard values.** On `/binder?person=<Alex>` at the `ipad-air` preset the header shows `Binder Value`, a DKK figure, `N unpriced.` and `Oldest price <date>.`, beside `DKK`, `EUR`, `USD` with DKK pressed. Open the `OP99-006` pocket: `Card Value`, a DKK figure, `Observed <date>` and a red `Stale` badge. Open `OP99-012`: no badge. Open Alex's `ja` pocket: no figure and "English printings only".
- **Dashboard currency.** Press `USD`: the header and an open sheet show US$ amounts and `document.cookie` holds `openhome-currency=USD`. Reload the tab: USD is still pressed and the figures are US$. `curl -s -H 'Cookie: openhome-currency=EUR' "$DASHBOARD_URL/binder?person=<Alex>"` has `binder-value-figure num">€…` and EUR `aria-pressed="true"` in the server-rendered HTML (no cookie: a DKK figure, and an unknown cookie value is DKK too).
- **Outage.** `down.sh`, then `OPENHOME_PREVIEW_ECB=outage up.sh`. The stack log has `ECB stand-in: GET /stats/eurofxref/eurofxref-daily.xml -> 503` and no rate for today: `OP99-005` (observed today) has a `rate_date` before today and the Binder reads still convert at it. Without the variable the same card's `rate_date` is today.
- **Proof.** Save each tool JSON as `01-…` onward, plus the stand-in log lines, under `$EVIDENCE_DIR/binder/`.

## Gotchas

- Add by the Card `id` from `search_cards`. Searching first is the recommended workflow, but an unsearched id is accepted when a Card Code inside it lists it. Run the unsearched-parallel step before any search of `OP99-002`, since a search stores the Cards.
- Card IDs are opaque catalogue image identifiers (`OP99-001_p1`, `EB99_OP99-020_P1`), stable across launches, unlike Household User IDs. A card's image file name can differ from its id.
- The synthetic images are flat colour drawings, so their AVIF sizes say nothing about real cards.
- The ECB stand-in's invented rates drift by day and the file carries only the latest working day, so a weekend run converts at Friday's rate. Prices observed before the 200-day history have `value_unavailable: "no_exchange_rates"`.
- The stand-in sits behind a loopback port that changes per launch; read it from the `OPTCG: loopback catalogue at <host:port>` line in the stack log if you need it. It prints no `http://`, because the Dashboard tests take the first `http://127.0.0.1:<port>` they read as Axum's address.
- The synthetic card pictures are flat colour drawings, so the Dashboard shows them as plain frames. Chrome shows the card's code and name only until the picture decodes.
