-- A Meal Log's optional photo (CONTEXT.md, Meal Log). The AVIF photo and its thumbnail are files
-- in the data directory's meal-photos/ named by photo_id; the database keeps only their ID and
-- pixel sizes. All five columns are set together or all are null.
ALTER TABLE meal_logs ADD COLUMN photo_id TEXT;
ALTER TABLE meal_logs ADD COLUMN photo_width INTEGER CHECK (photo_width > 0);
ALTER TABLE meal_logs ADD COLUMN photo_height INTEGER CHECK (photo_height > 0);
ALTER TABLE meal_logs ADD COLUMN photo_thumbnail_width INTEGER CHECK (photo_thumbnail_width > 0);
ALTER TABLE meal_logs ADD COLUMN photo_thumbnail_height INTEGER CHECK (photo_thumbnail_height > 0);
CREATE UNIQUE INDEX meal_logs_photo_id ON meal_logs (photo_id) WHERE photo_id IS NOT NULL;
