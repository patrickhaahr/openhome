ALTER TABLE cards ADD COLUMN inventory_price_usd REAL CHECK (inventory_price_usd IS NULL OR inventory_price_usd >= 0);
DROP TABLE exchange_rates;
