-- NOVA Classes (CONTEXT.md, NOVA Class) the agent gives Foods, by FoodID. The user's decision, so
-- it lives in app.db and survives a rebuild of food.db; Products keep Open Food Facts' class on
-- their own row.
CREATE TABLE food_nova_classes (
    food_id INTEGER PRIMARY KEY NOT NULL CHECK (food_id > 0),
    nova_class INTEGER NOT NULL CHECK (nova_class BETWEEN 1 AND 4),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- The class a Portion was logged with (ADR 0012): its Product's, or its Food's. Null while
-- unclassified; classifying the Food later fills the Food Portions of the same dataset version.
ALTER TABLE meal_log_portions ADD COLUMN nova_class INTEGER CHECK (nova_class BETWEEN 1 AND 4);
