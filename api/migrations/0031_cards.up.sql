-- One row per catalogue variant (a normal print or a parallel art), never edited after storing.
-- `id` is the catalogue's image identifier, kept opaque (OP01-001_p1, OP09-078-r1, EB03_OP09-034_p1);
-- `position` keeps catalogue order.
CREATE TABLE cards (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 64),
    card_code TEXT NOT NULL CHECK (length(card_code) BETWEEN 1 AND 20),
    position INTEGER NOT NULL CHECK (position >= 0),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 200),
    set_id TEXT,
    set_name TEXT,
    rarity TEXT,
    color TEXT,
    card_type TEXT,
    life TEXT,
    cost TEXT,
    power TEXT,
    counter_amount TEXT,
    attribute TEXT,
    sub_types TEXT,
    text TEXT,
    inventory_price_usd REAL CHECK (inventory_price_usd IS NULL OR inventory_price_usd >= 0),
    price_usd REAL CHECK (price_usd IS NULL OR price_usd >= 0),
    price_observed_on TEXT,
    image_avif BLOB NOT NULL CHECK (length(image_avif) > 0),
    CHECK ((price_usd IS NULL) = (price_observed_on IS NULL))
);
CREATE INDEX cards_code ON cards(card_code, position);

-- A Card Code the catalogue did not list on `checked_on` (a Copenhagen day): it is asked for again
-- on a later day, so a card the catalogue lists later can still be found.
CREATE TABLE unlisted_cards (
    card_code TEXT PRIMARY KEY NOT NULL CHECK (length(card_code) BETWEEN 1 AND 20),
    checked_on TEXT NOT NULL CHECK (checked_on GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]')
);
