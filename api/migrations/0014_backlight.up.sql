CREATE TABLE backlight (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    tv_sync_profile TEXT,
    intended TEXT NOT NULL DEFAULT '{"mode":"off"}'
);
INSERT INTO backlight (id) VALUES (1);
