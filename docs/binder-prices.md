# Binder prices: daily refresh and Price History

Part of [Binder](binder.md). Terms (Card Price, Price History) are in [`context/binder.md`](context/binder.md).

## Data

`price_points` (migration 0034) holds one row per Card and observation date: `card_id`,
`observed_on`, `price_usd_cents` (integer). `cards.price_usd` and `price_observed_on` stay the
latest price, and `price_observed_on` always equals the newest point's date.

- The first point is written in the same transaction that stores the Card (`cards::store`),
  whichever path stored it. Nothing is backfilled, including from the catalogue's two-week endpoint.
- Migration 0034 seeds one point per existing priced Card from its stored latest price and date.
  Those Cards' first stored price is exactly that, and the catalogue cannot supply it honestly
  later. A Card stored with no price has no point until the refresh sees one.
- The ECB backfill reach (`exchange_rates::needs_history`) now measures the oldest Price Point,
  since a point can be older than any Card's latest price.

## Daily refresh

`services/price_refresh` runs one pass a day, however often the process restarts. The next due
time is one row in `price_refresh_state` (migration 0035). A pass moves it 24 hours ahead when it
starts, not when it ends, so a crash loop asks the catalogue once and a restart waits for the due
time. A pass the database failed moves it back to 15 minutes ahead. With no row the first start
runs a pass. (The ECB job still runs at every start.) A pass asks the catalogue once per distinct Card Code
that has a Binder Entry, two seconds apart, and for each owned variant of the answer runs one
guarded `UPDATE ... WHERE price_observed_on < ?` plus the point insert in one transaction. An
unchanged or older observation date writes nothing, so retries and simultaneous runs append each
observation once; runs also take one lock, so pacing holds across them. A catalogue failure is
logged with the Card Code only, counted, and the run goes on. No log line from the adapter
carries catalogue text: it names a reason, a Card Code or a field. Unowned Cards are never asked for or
touched, so they keep their details, image and history. A sibling variant nobody owns is not
updated either.

## Read

`GET /api/binder/cards/{id}/prices?from=&to=` (MCP `card_prices`, read-only) returns
`{card_id, prices: [{observed_on, usd, eur, dkk, rate_date}]}` oldest first, `from` and `to`
inclusive `YYYY-MM-DD`, each point converted by `card_value::convert` at its own date's rates. With
no rate at or before a point, `eur`, `dkk` and `rate_date` are null. Unknown id 404, malformed
range 400.

## Dashboard

The card sheet of an English entry reads the history when it opens and draws it with `TrendChart`
in the selected currency. The series has one slot per day from the first to the last point, and a
day without a point is a null slot, so the line breaks (a lone point is a dot). A point with no
rate in the chosen currency is a null slot too, and the table says "No exchange rate" there
instead of "No observation". A Japanese entry
shows no chart.

## Verification

`api/tests/price_history.rs`, `mcp/tests/binder.rs` (`card_prices`), `dashboard/tests/binder-prices.test.tsx`
and the recipe [`binder-prices.md`](../.agents/skills/verify-openhome/features/binder-prices.md).
