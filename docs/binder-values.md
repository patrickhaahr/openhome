# Card Value, Binder Value and the ECB rates

Terms are in [`docs/context/binder.md`](context/binder.md); the catalogue is in
[`binder.md`](binder.md). The household reads values in Danish kroner (DKK) unless it
asks for euros (EUR) or US dollars (USD).

## Rates

`services/ecb` is the only code that talks to the European Central Bank. It reads two
files (replaceable only in code, for tests and the preview), both with no redirects and the
app's `User-Agent`. The 90-day file, `eurofxref-hist-90d.xml`, holds about 64 working days
(roughly 100 KB), so it has a five second timeout and a 1 MiB limit. The history file, `eurofxref-hist.xml`, holds every working day back to 1999
(about 8 MB on one line), so it has a 60 second timeout and a 16 MiB limit. Both quote every
currency per one euro. OpenHome keeps USD and DKK, so USD per EUR is the USD rate and DKK per
EUR the DKK rate.

The parser is strict: the document must be `Envelope > Cube > Cube[time] >
Cube[currency, rate]`, with no DTD, each currency at most once per day and every kept rate a
positive decimal of at most six decimals. Both files are read by one parser, as a stream of
events, never a DOM. A day that lacks USD or DKK is skipped (very old days lack some
currencies), a file with no complete day and a malformed file are refused whole.
Anything else, and every network failure, is an `Unavailable` that only reaches the log.

`services/exchange_rates` stores one row per ECB date in `exchange_rates` (migration
0033, rates as integer micro-units). Each pass reads the history file when the stored rates
cannot be completed from the 90-day file: there are no rates, no stored day at or before the
oldest stored Card observation date (`MIN(cards.price_observed_on)`), or the newest stored
day is more than 80 days old (the margin keeps the 90-day file's first day at or before it).
Otherwise it reads the 90-day file and upserts every day it holds, so a working day the
service missed, such as an outage or a restart after one, is filled by the next pass, for
any gap under 90 days. The ticket's "fetches the ECB reference-rate file and stores that
day's USD and DKK rates" still holds: each pass stores the current day among the days it
reads. A pass runs at startup and every 24 hours, like the feed scheduler, and after 15
minutes when it failed. A pass writes all its days in one transaction, so a failed pass
writes nothing and the previous rates stay in use. A day's row is replaced by a later read of
the same date. After one backfill the earliest stored day is 1999-01-04, so the history file
is not read again unless a Card was observed before that or the service was down for most of
a quarter.

## Conversion

`services/card_value::convert` is pure. A USD price observed on a day is converted at
the rates of that day, or of the latest earlier stored day when the ECB published none
(weekends, holidays, outages), and the result names the `rate_date` it used. EUR is the
price over the USD rate. DKK is the price times the DKK rate over the USD rate. Money is
integer cents and each result is rounded once, half away from zero, from the exact
quotient. Sums use the cent amounts, never floats.

## What reads carry

Every Card and Binder Entry read has either `card_value` or `value_unavailable`:

```json
"card_value": { "usd": 2.25, "eur": 2.03, "dkk": 15.14,
  "observed_on": "2026-11-12", "rate_date": "2026-11-12", "stale": false },
"value_unavailable": null
```

`stale` is true when the price was observed more than 30 days before today in
Europe/Copenhagen. `value_unavailable` is `japanese_printing` (the catalogue prices
English printings only), `no_catalogue_price` or `no_exchange_rates` (no stored rates at or
before the price's day), and `card_value` is then `null`.

`GET /api/binder/entries` returns `{ "entries": [...], "binder_values": [...] }`.
`binder_values` has one item per Household User in scope (every one, or the `owner_id`
filter's), so an empty Binder is worth zero: `owner_id`, `usd`, `eur`,
`dkk`, `unpriced_entries` and `oldest_observed_on`. A Binder Value is quantity times
Card Value over the priced entries in each currency, stale prices included. Entries
without a value are counted in `unpriced_entries`, and the oldest observation date is
among the priced ones (null when none is priced).

A retried addition replays the stored receipt exactly as it was recorded, plus
`replayed: true`: the stored JSON is returned without being decoded, so its `card_value`
(or `value_unavailable`) is the snapshot taken when it was added, including its `stale` flag,
whatever the rates or the Card's price are by then. A fresh read is current. A receipt
written before Card Value existed replays the same way, with `card_value` as `{usd,
observed_on}` or null and no `value_unavailable`; the receipt row is never rewritten.

## Dashboard

The Binder page shows the Binder Value, with the unpriced count and the oldest price date,
beside a DKK, EUR and USD switch. A card sheet shows its Card Value, the observation day
and a Stale badge, or why there is none. The choice is the `openhome-currency` cookie
(one year, per device), read on the server so the first paint is already in that
currency, and DKK when it is absent or unknown.
