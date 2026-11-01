-- A FoodID names the same Food only within one dataset version, so each NOVA Class records the
-- dataset version it was given in and is read only while food.db holds that version; a rebuild of
-- the same version keeps it. Existing classes were given in DTU 6.1, the only version imported so
-- far.
CREATE TABLE food_nova_classes_by_dataset (
    food_id INTEGER PRIMARY KEY NOT NULL CHECK (food_id > 0),
    dataset_version TEXT NOT NULL CHECK (dataset_version <> ''),
    nova_class INTEGER NOT NULL CHECK (nova_class BETWEEN 1 AND 4),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO food_nova_classes_by_dataset (food_id, dataset_version, nova_class, created_at, updated_at)
SELECT food_id, '6.1', nova_class, created_at, updated_at FROM food_nova_classes;

DROP TABLE food_nova_classes;

ALTER TABLE food_nova_classes_by_dataset RENAME TO food_nova_classes;
