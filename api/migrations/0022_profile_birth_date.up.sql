-- The Profile gains an optional birth date (CONTEXT.md, Profile): the BMR formula behind the
-- nutrition targets and the NNR age bands need it.

ALTER TABLE profile ADD COLUMN birth_date TEXT
    CHECK (birth_date IS NULL OR birth_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]');
