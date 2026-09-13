CREATE TABLE exercises (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    category TEXT NOT NULL CHECK (category IN ('calisthenics', 'gym')),
    muscle_group TEXT,
    equipment TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE workouts (
    id INTEGER PRIMARY KEY,
    date DATE NOT NULL,
    name TEXT,
    notes TEXT,
    body_weight_kg REAL,
    sleep_hours REAL,
    energy_level INTEGER,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE workout_exercises (
    id INTEGER PRIMARY KEY,
    workout_id INTEGER NOT NULL REFERENCES workouts(id) ON DELETE CASCADE,
    exercise_id INTEGER NOT NULL REFERENCES exercises(id),
    order_index INTEGER NOT NULL,
    notes TEXT
);

CREATE TABLE sets (
    id INTEGER PRIMARY KEY,
    workout_exercise_id INTEGER NOT NULL REFERENCES workout_exercises(id) ON DELETE CASCADE,
    set_number INTEGER NOT NULL,
    reps INTEGER,
    weight_kg REAL,
    duration_seconds INTEGER,
    rpe INTEGER CHECK (rpe IS NULL OR (rpe >= 1 AND rpe <= 10)),
    notes TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE body_weight (
    id INTEGER PRIMARY KEY,
    date DATE NOT NULL UNIQUE,
    weight_kg REAL NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    height_cm REAL,
    sex TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX workout_exercises_workout_id_idx ON workout_exercises(workout_id);
CREATE INDEX workout_exercises_exercise_id_idx ON workout_exercises(exercise_id);
CREATE INDEX sets_workout_exercise_id_idx ON sets(workout_exercise_id);
CREATE INDEX workouts_date_idx ON workouts(date);

INSERT INTO exercises (name, category, muscle_group, equipment) VALUES
    -- Calisthenics
    ('Planche Hold', 'calisthenics', 'core', 'bodyweight'),
    ('Planche Push-up', 'calisthenics', 'chest', 'bodyweight'),
    ('Planche Press', 'calisthenics', 'chest', 'bodyweight'),
    ('Front Lever Hold', 'calisthenics', 'back', 'bodyweight'),
    ('Front Lever Pull-up', 'calisthenics', 'back', 'bodyweight'),
    ('Front Lever Raise', 'calisthenics', 'back', 'bodyweight'),
    ('Front Lever Touch', 'calisthenics', 'back', 'bodyweight'),
    ('Handstand Push-up', 'calisthenics', 'shoulders', 'bodyweight'),
    ('Handstand (timed)', 'calisthenics', 'shoulders', 'bodyweight'),
    ('Ring Dip', 'calisthenics', 'triceps', 'rings'),
    ('Pull-up', 'calisthenics', 'back', 'bodyweight'),
    ('Dip', 'calisthenics', 'triceps', 'bodyweight'),
    -- Gym: chest
    ('Converging Chest Press', 'gym', 'chest', 'machine'),
    ('Incline Chest Press Machine', 'gym', 'chest', 'machine'),
    -- Gym: shoulders
    ('Side Delt Raise Cable', 'gym', 'shoulders', 'cable'),
    ('Side Delt Raise Machine', 'gym', 'shoulders', 'machine'),
    ('Rear Delt Machine', 'gym', 'shoulders', 'machine'),
    ('Shoulder Press Machine', 'gym', 'shoulders', 'machine'),
    -- Gym: biceps
    ('Preacher Biceps Machine', 'gym', 'biceps', 'machine'),
    ('Biceps Curl Cable', 'gym', 'biceps', 'cable'),
    -- Gym: triceps
    ('Triceps Pushdown', 'gym', 'triceps', 'cable'),
    ('Overhead Triceps Extension Cable', 'gym', 'triceps', 'cable'),
    -- Gym: back
    ('Seal Row', 'gym', 'back', 'barbell'),
    ('T-bar Row', 'gym', 'back', 'barbell'),
    ('Lat Pull-down', 'gym', 'back', 'cable'),
    ('Chest Supported Row Machine', 'gym', 'back', 'machine'),
    -- Gym: glutes
    ('Glute Bridge Machine', 'gym', 'glutes', 'machine'),
    ('Hip Abduction Machine', 'gym', 'glutes', 'machine'),
    ('Seated Hip Abduction Machine', 'gym', 'glutes', 'machine'),
    ('Cable Kickback', 'gym', 'glutes', 'cable');
