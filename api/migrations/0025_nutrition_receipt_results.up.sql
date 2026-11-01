-- Results are captured in the accepting transaction. Older receipts cannot recover an original
-- result after edits, so remain null and return a conflict directing the client to read first.
ALTER TABLE meal_log_receipts ADD COLUMN result TEXT CHECK (result IS NULL OR json_valid(result));
ALTER TABLE meal_receipts ADD COLUMN result TEXT CHECK (result IS NULL OR json_valid(result));
ALTER TABLE supplement_receipts ADD COLUMN result TEXT CHECK (result IS NULL OR json_valid(result));
ALTER TABLE product_link_receipts ADD COLUMN result TEXT CHECK (result IS NULL OR json_valid(result));
