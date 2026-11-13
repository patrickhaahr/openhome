# Card Value, Binder Value and the ECB rates

Terms are in [`docs/context/binder.md`](context/binder.md); the catalogue is in
[`binder.md`](binder.md). The household reads values in Danish kroner (DKK) unless it
asks for euros (EUR) or US dollars (USD).

## Rates

`services/ecb` is the only code that talks to the European Central Bank. It reads two
files (replaceable only in code, for tests and the preview), both with no redirects and the
app's `User-Agent`. The daily file, `eurofxref-daily.xml`, has a five second timeout and a
64 KiB limit. The history file, `eurofxref-hist.xml`, holds every working day back to 1999
(about 8 MB on one line), so it has a 60 second timeout and a 16 MiB limit. Both quote every
currency per one euro. OpenHome keeps USD and DKK, so USD per EUR is the USD rate and DKK per
EUR the DKK rate.

The parser is strict: the document must be `Envelope > Cube > Cube[time] >
Cube[currency, rate]`, with no DTD, each currency at most once per day and every kept rate a
positive decimal of at most six decimals. The daily file must hold one date with USD and DKK.
The history file is read as a stream of events, never a DOM, and a day that lacks USD or DKK
is skipped (very old days lack some currencies) while a malformed file is refused whole.
Anything else, and every network failure, is an `Unavailable` that only reaches the log.

`services/exchange_rates` stores one row per ECB date in `exchange_rates` (migration
0033, rates as integer micro-units). Each pass reads the history file when the stored rates
do not cover the oldest stored Card observation date (`MIN(cards.price_observed_on)`): there
are no rates, or no stored day at or before that date. Otherwise it reads the daily file. A
pass runs at startup and every 24 hours, like the feed scheduler, and after 15 minutes when
it failed. A pass writes all its days in one transaction, so a failed pass writes nothing and
the previous rates stay in use. A day's row is replaced by a later read of the same date.
After one backfill the earliest stored day is 1999-01-04, so the history file is not read
again unless a Card was observed before that.

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
`binder_values` has one item per owner that has entries: `owner_id`, `usd`, `eur`,
`dkk`, `unpriced_entries` and `oldest_observed_on`. A Binder Value is quantity times
Card Value over the priced entries in each currency, stale prices included. Entries
without a value are counted in `unpriced_entries`, and the oldest observation date is
among the priced ones (null when none is priced).

A retried addition replays the stored receipt, so its `card_value` is the snapshot taken
when it was added, including its `stale` flag. A fresh read is current. A receipt written
before Card Value existed holds `card_value` as `{usd, observed_on}` or null and no
`value_unavailable`, which holds no EUR or DKK. Its replay keeps every other stored field and
takes the Card's value as of now (a Japanese entry stays `japanese_printing`); the receipt row
is never rewritten.

## Dashboard

The Binder page shows the Binder Value, with the unpriced count and the oldest price date,
beside a DKK, EUR and USD switch. A card sheet shows its Card Value, the observation day
and a Stale badge, or why there is none. The choice is the `openhome-currency` cookie
(one year, per device), read on the server so the first paint is already in that
currency, and DKK when it is absent or unknown.
