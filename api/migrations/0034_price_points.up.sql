-- One Card Price per observation date, in USD cents. The first row is written when the Card is
-- first stored; the daily refresh appends a row only when the catalogue's observation date is
-- newer than the Card's latest one, so `cards.price_observed_on` is always the newest row's date.
-- A Card's history outlives its Binder Entries.
CREATE TABLE price_points (
    card_id TEXT NOT NULL REFERENCES cards(id),
    observed_on TEXT NOT NULL CHECK (observed_on GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    price_usd_cents INTEGER NOT NULL CHECK (price_usd_cents >= 0),
    PRIMARY KEY (card_id, observed_on)
) WITHOUT ROWID;

-- Cards stored before this table existed have no recorded first price, and the catalogue cannot
-- supply one honestly, so the stored latest price and its date stand in as that first point.
INSERT INTO price_points (card_id, observed_on, price_usd_cents)
SELECT id, price_observed_on, CAST(ROUND(price_usd * 100) AS INTEGER)
FROM cards
WHERE price_usd IS NOT NULL;
