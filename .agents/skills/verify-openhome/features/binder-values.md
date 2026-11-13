# Binder values

Hermes and the household read each Card in DKK, EUR and USD, and each Binder's total, from the ECB's reference rates. This recipe extends [binder.md](./binder.md): same stack, same invented `OP99` cards, same seeded owners.

## Sub-features

- `binder-value-mcp` reads `card_value` in three currencies, `stale` flags, an unpriced `ja` entry and `binder_values` through MCP.
- `binder-value-dash` shows the Binder Value in the header and the Card Value with its date and Stale badge in the card sheet.
- `binder-value-currency` switches DKK, EUR and USD; the choice survives a reload, and the initial HTML already holds it.
- `binder-value-backfill` reads the first scheduler pass's backfill from the stand-in's history file: the parallel `OP99-001_p1`, priced five months ago, is valued at its own day's rate and counts as stale.
- `binder-value-outage` starts with `OPENHOME_PREVIEW_ECB=outage`: no rates arrive, so every price is `no_exchange_rates`.
- `binder-value-missed-day` starts with `OPENHOME_PREVIEW_ECB=missed-day`: the history file lacks one working day, and the next pass, with the 90-day file, fills it. A retry made before that pass still replays its original value.

## Driving it with verify-openhome

Preconditions: those of `binder.md`, plus the ECB stand-in.

- The ECB stand-in serves the 90-day file and the history file at their production paths from a loopback port, logged as `ECB stand-in: GET <path> -> <status>` (the port is on the `ECB: loopback reference rates at <host:port>` line). The history file holds invented rates for every working day of the last 365 days, and the first scheduler pass reads it because the stack starts with no stored rates, and the stand-in logs `ECB stand-in: history holds <N> working days, <oldest> to <newest>`. The 90-day file holds the working days of the last 90 calendar days. Start with `OPENHOME_PREVIEW_ECB=outage` or `missed-day` in the environment of `up.sh` for those steps. Never contact the live bank.

- **Values.** `mcp.sh call binder_entries '{"owner_id":"<Alex>"}'` returns `entries` and `binder_values` (one item for Alex: `usd`, `eur`, `dkk`, `unpriced_entries`, `oldest_observed_on`). Alex's `OP99-006` price is 45 days old and `OP99-015` 31 days old: `stale: true`, and `OP99-012` is fresh: `stale: false`. Alex's `ja` entries have `card_value: null` and `value_unavailable: "japanese_printing"`, and count in `unpriced_entries`. Each priced entry's `rate_date` is the working day on or before its `observed_on`. The `dkk`, `eur` and `usd` totals equal quantity times each card's value summed over the priced entries, including stale ones, to the cent.
- **Dashboard values.** On `/binder?person=<Alex>` at the `ipad-air` preset the header shows `Binder Value`, a DKK figure, `N unpriced.` and `Oldest price <date>.`, beside `DKK`, `EUR`, `USD` with DKK pressed. Open the `OP99-006` pocket: `Card Value`, a DKK figure, `Observed <date>` and a red `Stale` badge. Open `OP99-012`: no badge. Open Alex's `ja` pocket: no figure and "English printings only".
- **Dashboard currency.** Press `USD`: the header and an open sheet show US$ amounts and `document.cookie` holds `openhome-currency=USD`. Reload the tab: USD is still pressed and the figures are US$. `curl -s -H 'Cookie: openhome-currency=EUR' "$DASHBOARD_URL/binder?person=<Alex>"` has `binder-value-figure num">€…` and EUR `aria-pressed="true"` in the server-rendered HTML (no cookie: a DKK figure, and an unknown cookie value is DKK too).
- **Backfill.** On a fresh launch the stack log has `ECB stand-in: GET /stats/eurofxref/eurofxref-hist.xml -> 200` once and no 90-day request, preceded by the `history holds <N> working days` line (about 250 days back to a year ago). `mcp.sh call search_cards '{"card_code":"OP99-001"}'` gives `OP99-001_p1` a `card_value` whose `observed_on` is five months old (2026-06-09 on 2026-11-13), `rate_date` equal to it (or the working day before, on a weekend) and `stale: true`. The rate is the stand-in's invented one for that day, so the DKK and EUR amounts are the price times that day's rates. Add `OP99-001_p1` to Sam's Binder and read `binder_entries` for Sam: `binder_values` counts it (`unpriced_entries` 0, `oldest_observed_on` that day) and `usd`, `eur` and `dkk` equal its `card_value`. Alex's `OP99-006` (45 days old) is valued at its own day too.
- **Outage.** `down.sh`, then `OPENHOME_PREVIEW_ECB=outage up.sh`. The stack log has `ECB stand-in: GET /stats/eurofxref/eurofxref-hist.xml -> 503`, no stored rates, and every Card has `card_value: null` with `value_unavailable: "no_exchange_rates"`. Without the variable the same cards are valued.
- **Missed day.** `OPENHOME_PREVIEW_ECB=missed-day up.sh`. The history line ends `without <day>`, the working day on or before 31 days ago, which is Alex's `OP99-015` price day. Within 90 seconds of the API starting, read `binder_entries` for Alex: `OP99-015` has `rate_date` the working day before the missing one. Add `OP99-015` to Sam with a `request_id` and keep the response. About 90 seconds after start the log has `ECB stand-in: the next day's pass`, `GET /stats/eurofxref/eurofxref-hist-90d.xml -> 200` and `stored Recent { days: 65 }` (about 65 working days), and no second history request. Read Alex's `binder_entries` again: `OP99-015` now has `rate_date` equal to its `observed_on`, and its EUR may differ by a cent. Repeat the Sam addition: `replayed: true` and an `entry` identical to the first response, with the old `rate_date`, while Sam's `binder_entries` shows the new one.
- **Proof.** Save each tool JSON as `01-…` onward, plus the stand-in log lines, under `$EVIDENCE_DIR/binder/`.

## Gotchas

- The ECB stand-in's invented rates drift by day. A weekend run converts at the latest working day's (Friday's) rate. The history file covers the last 365 days, so only a price older than that has `value_unavailable: "no_exchange_rates"`. The scheduler retries a failed pass after 15 minutes, which the recipe does not wait for.
