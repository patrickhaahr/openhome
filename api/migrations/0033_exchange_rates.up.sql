-- The ECB's reference rates per euro for one working day, as integers with six decimals
-- (1.1081 is 1108100), so a conversion never meets a float. A day is written once the ECB file
-- for it is read; a failed read writes nothing.
CREATE TABLE exchange_rates (
    rate_date TEXT PRIMARY KEY NOT NULL CHECK (rate_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    usd_micros INTEGER NOT NULL CHECK (usd_micros > 0),
    dkk_micros INTEGER NOT NULL CHECK (dkk_micros > 0)
);

-- The catalogue's inventory price was never shown anywhere; Card Value uses the market price.
ALTER TABLE cards DROP COLUMN inventory_price_usd;
