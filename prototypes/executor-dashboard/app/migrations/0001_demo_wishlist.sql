-- Synthetic prototype data only; no OpenHome production database access.
CREATE TABLE wishlist_items (
  id TEXT PRIMARY KEY NOT NULL,
  owner TEXT NOT NULL CHECK (owner IN ('Alex', 'Riley')),
  title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 160)
);
INSERT INTO wishlist_items VALUES ('demo-1', 'Alex', 'A light backpack for day walks');
INSERT INTO wishlist_items VALUES ('demo-2', 'Alex', 'A good pair of running socks');
INSERT INTO wishlist_items VALUES ('demo-3', 'Riley', 'A little reading lamp');
