-- Household-shared Important Dates: no owner; 0..n Household Users are tagged instead.
CREATE TABLE important_dates (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 200),
    date TEXT NOT NULL CHECK (date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    kind TEXT NOT NULL CHECK (kind IN ('since', 'yearly')),
    show_when_idle INTEGER NOT NULL DEFAULT 0 CHECK (show_when_idle IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- A removed date or Household User removes its tags.
CREATE TABLE important_date_people (
    date_id TEXT NOT NULL REFERENCES important_dates(id) ON DELETE CASCADE,
    person_id TEXT NOT NULL REFERENCES household_users(id) ON DELETE CASCADE,
    PRIMARY KEY (date_id, person_id)
);
CREATE INDEX important_date_people_person ON important_date_people(person_id);

-- No foreign key: accepted intentions remain retired after the date is deleted.
CREATE TABLE important_date_addition_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    date_id TEXT NOT NULL,
    title TEXT NOT NULL,
    date TEXT NOT NULL,
    kind TEXT NOT NULL,
    person_ids TEXT NOT NULL,
    show_when_idle INTEGER NOT NULL
);
