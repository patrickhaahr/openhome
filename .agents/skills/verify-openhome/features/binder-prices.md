# Binder prices

Owned Cards' prices refresh daily and each Card builds a Price History. Hermes reads it with `card_prices` and the Dashboard card sheet draws it. This recipe extends [binder.md](./binder.md): same stack, same invented `OP99` cards.

## Sub-features

- `binder-price-refresh` shows the real refresh job asking once per owned Card Code and appending only newer observations.
- `binder-price-failure` shows one code failing (503) while the rest of the pass goes on.
- `binder-price-mcp` reads a Price History through `card_prices`, with a range, and matches the API read.
- `binder-price-errors` reads an unknown id (404) and a reversed range (400).
- `binder-price-chart` shows the price line in the card sheet at the `ipad-air` preset, with gaps.

## How to get to it (user POV)

- Ask Hermes how a card's price has moved (MCP tool `card_prices`).
- Open `/binder`, tap an English pocket and read `Price History` in the sheet; the currency switch changes the line.

## Driving it with verify-openhome

Preconditions: those of `binder.md`, started with `OPENHOME_PREVIEW_PRICES=history up.sh`. That moves the OPTCG stand-in's observations 12 days earlier and, after seeding, replays days 1 to 12 through the real refresh job: days 5 to 7 are skipped (OpenHome "not running"), and on day 9 the stand-in answers 503 for `OP99-013`. The stand-in port is on the `OPTCG: loopback catalogue` line.

- **Refresh.** `grep "Price refresh stand-in" "$RUN_DIR/logs/stack.log"` has `day 1` to `day 12` without 5, 6 and 7, each `Pass { codes: 9, ... }`: nine owned codes, and the `OPTCG stand-in: GET /api/sets/card/<code>/` lines between two pass lines number nine, one per code. `appended: 11` is the owned variants. The last line, `unchanged observations`, has `appended: 0`. Unowned `OP99-001` and `OP99-002` are never requested.
- **Failure.** The `day 9` pass has one `OP99-013 -> 503` line, `failed: 1`, `appended: 9`, and the codes after it were still asked.
- **MCP.** `mcp.sh call card_prices '{"card_id":"OP99-013"}'` returns nine points: usd, eur and dkk, each with the `rate_date` of its own day (or the latest earlier working day), with no points for the skipped days or the failed day. Add `"from":"2026-11-03","to":"2026-11-09"` (days relative to today) for the range, and compare with `curl -H "Authorization: Bearer $API_KEY" "$API_URL/api/binder/cards/OP99-013/prices?from=...&to=..."`: same values.
- **Errors.** `card_id` `OP99-404` exits 2 with status 404, a `from` after `to` exits 2 with status 400.
- **Chart.** Open `/binder?person=<Alex>` in a tab with `open: true`, apply `preview_resize` `ipad-air` landscape, open the `OP99-013` pocket and scroll the sheet to `Price History`: the summary reads `9 observations since <day>, 4 days without an observation`, and the line breaks at the gaps with the day between them as a lone dot. Press `EUR`: the axis and table change. Open Alex's Japanese `OP99-004` pocket: no chart.
- **Proof.** Save the JSON, the stack log lines and a screenshot under `$EVIDENCE_DIR/binder/`.

## Gotchas

- Without `OPENHOME_PREVIEW_PRICES=history` every Card has the one first point and no refresh runs, since the preview does not start the 24-hour scheduler.
- The preview's pass uses no pacing delay; production waits two seconds between codes.
