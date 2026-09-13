import { isJsonArray, isJsonObject, isJsonNumber, isJsonString, type Json } from "./json";
import { failure, success, type Result } from "./result";

/** One exercise in the library, as served by the Axum API. */
export type Exercise = {
  readonly id: number;
  readonly name: string;
  readonly category: string;
  readonly muscleGroup: string | null;
  readonly equipment: string | null;
};

/** The categories the Axum API accepts for an exercise. */
export const exerciseCategories = ["calisthenics", "gym"] as const;
export type ExerciseCategory = (typeof exerciseCategories)[number];

/** A new exercise as entered in the create form, ready for the Axum API. */
export type ExerciseInput = {
  readonly name: string;
  readonly category: ExerciseCategory;
  readonly muscleGroup: string | null;
  readonly equipment: string | null;
};

export const EXERCISE_READ_ERROR = "Couldn't read the exercise from the Axum API.";
export const EXERCISE_LIST_READ_ERROR = "Couldn't read the exercise library from the Axum API.";
export const WORKOUT_READ_ERROR = "Couldn't read the workout from the Axum API.";
export const WORKOUT_LIST_READ_ERROR = "Couldn't read the workouts from the Axum API.";
export const PROGRESS_READ_ERROR = "Couldn't read the exercise progress from the Axum API.";
export const BODY_WEIGHT_READ_ERROR = "Couldn't read the body weight entries from the Axum API.";
export const PROFILE_READ_ERROR = "Couldn't read the profile from the Axum API.";

/**
 * Parse an untrusted exercise payload. The Axum API serves exercises as
 * `{ id, name, category, muscle_group, equipment }` with optional fields null.
 */
export function parseExercise(json: Json | undefined): Result<Exercise> {
  if (
    !isJsonObject(json) ||
    !isJsonNumber(json["id"]) ||
    !isJsonString(json["name"]) ||
    !isJsonString(json["category"])
  ) {
    return failure(EXERCISE_READ_ERROR);
  }
  return success({
    id: json["id"],
    name: json["name"],
    category: json["category"],
    muscleGroup: optionalJsonText(json["muscle_group"]),
    equipment: optionalJsonText(json["equipment"]),
  });
}

/** Parse an untrusted exercise list response; any non-conforming entry rejects the payload. */
export function parseExerciseList(json: Json): Result<readonly Exercise[]> {
  if (!isJsonArray(json)) {
    return failure(EXERCISE_LIST_READ_ERROR);
  }
  const exercises: Exercise[] = [];
  for (const raw of json) {
    const exercise = parseExercise(raw);
    if (!exercise.ok) {
      return failure(EXERCISE_LIST_READ_ERROR);
    }
    exercises.push(exercise.value);
  }
  return success(exercises);
}

/** Validate a create-exercise form on the client before any request is sent. */
export function parseExerciseInput(
  name: string,
  category: string,
  muscleGroup: string,
  equipment: string,
): Result<ExerciseInput> {
  const trimmedName = name.trim();
  if (trimmedName.length === 0) {
    return failure("Enter a name for the exercise.");
  }
  if (!exerciseCategories.includes(category as ExerciseCategory)) {
    return failure("Choose calisthenics or gym as the category.");
  }
  return success({
    name: trimmedName,
    category: category as ExerciseCategory,
    muscleGroup: trimmedOrNull(muscleGroup),
    equipment: trimmedOrNull(equipment),
  });
}

/** Which exercises the library shows: free-text name search plus category and muscle group filters. */
export function filterExercises(
  exercises: readonly Exercise[],
  query: string,
  category: ExerciseCategory | "all",
  muscleGroup: string,
): readonly Exercise[] {
  const needle = query.trim().toLowerCase();
  const muscleNeedle = muscleGroup.trim().toLowerCase();
  return exercises.filter((exercise) => {
    if (!exercise.name.toLowerCase().includes(needle)) {
      return false;
    }
    if (category !== "all" && exercise.category !== category) {
      return false;
    }
    if (
      muscleNeedle.length > 0 &&
      (exercise.muscleGroup === null || !exercise.muscleGroup.toLowerCase().includes(muscleNeedle))
    ) {
      return false;
    }
    return true;
  });
}

/** Trim optional JSON text, mapping absent or blank values to null. */
function optionalJsonText(value: Json | undefined): string | null {
  if (!isJsonString(value)) {
    return null;
  }
  return trimmedOrNull(value);
}

/** One set as served inside a workout detail. Reps and duration are mutually optional. */
export type WorkoutSet = {
  readonly id: number;
  readonly setNumber: number;
  readonly reps: number | null;
  readonly weightKg: number | null;
  readonly durationSeconds: number | null;
  readonly rpe: number | null;
  readonly notes: string | null;
};

/** One exercise entry as served inside a workout detail, in the order performed. */
export type WorkoutExerciseEntry = {
  readonly id: number;
  readonly orderIndex: number;
  readonly notes: string | null;
  readonly exercise: { readonly id: number; readonly name: string; readonly category: string };
  readonly sets: readonly WorkoutSet[];
};

/** A workout with its embedded exercises and sets. */
export type WorkoutDetail = {
  readonly id: number;
  readonly date: string;
  readonly name: string | null;
  readonly notes: string | null;
  readonly bodyWeightKg: number | null;
  readonly exercises: readonly WorkoutExerciseEntry[];
};

/** A compact workout as served by the history list. */
export type WorkoutSummary = {
  readonly id: number;
  readonly date: string;
  readonly name: string | null;
};

/** One set as entered in the log form, ready for the Axum API. */
export type SetInput = {
  readonly setNumber: number;
  readonly reps: number | null;
  readonly weightKg: number | null;
  readonly durationSeconds: number | null;
  readonly rpe: number | null;
  readonly notes: string | null;
};

/** One exercise entry as entered in the log form, ready for the Axum API. */
export type WorkoutExerciseInput = {
  readonly exerciseId: number;
  readonly notes: string | null;
  readonly sets: readonly SetInput[];
};

/** A new workout as entered in the log form, ready for the Axum API. */
export type WorkoutInput = {
  readonly date: string;
  readonly name: string | null;
  readonly notes: string | null;
  readonly bodyWeightKg: number | null;
  readonly exercises: readonly WorkoutExerciseInput[];
};

/**
 * Parse an untrusted set payload as served by the Axum API:
 * `{ id, set_number, reps, weight_kg, duration_seconds, rpe, notes }`.
 */
export function parseWorkoutSet(json: Json): Result<WorkoutSet> {
  if (
    !isJsonObject(json) ||
    !isJsonNumber(json["id"]) ||
    !isJsonNumber(json["set_number"]) ||
    !isOptionalJsonNumber(json["reps"]) ||
    !isOptionalJsonNumber(json["weight_kg"]) ||
    !isOptionalJsonNumber(json["duration_seconds"]) ||
    !isOptionalJsonNumber(json["rpe"])
  ) {
    return failure(WORKOUT_READ_ERROR);
  }
  return success({
    id: json["id"],
    setNumber: json["set_number"],
    reps: optionalJsonNumber(json["reps"]),
    weightKg: optionalJsonNumber(json["weight_kg"]),
    durationSeconds: optionalJsonNumber(json["duration_seconds"]),
    rpe: optionalJsonNumber(json["rpe"]),
    notes: optionalJsonText(json["notes"]),
  });
}

/** Parse an untrusted workout exercise entry with its embedded sets. */
export function parseWorkoutExerciseEntry(json: Json): Result<WorkoutExerciseEntry> {
  if (!isJsonObject(json) || !isJsonNumber(json["id"]) || !isJsonNumber(json["order_index"])) {
    return failure(WORKOUT_READ_ERROR);
  }
  const exercise = json["exercise"];
  if (
    !isJsonObject(exercise) ||
    !isJsonNumber(exercise["id"]) ||
    !isJsonString(exercise["name"]) ||
    !isJsonString(exercise["category"])
  ) {
    return failure(WORKOUT_READ_ERROR);
  }
  if (!isJsonArray(json["sets"])) {
    return failure(WORKOUT_READ_ERROR);
  }
  const sets: WorkoutSet[] = [];
  for (const raw of json["sets"]) {
    const set = parseWorkoutSet(raw);
    if (!set.ok) {
      return set;
    }
    sets.push(set.value);
  }
  return success({
    id: json["id"],
    orderIndex: json["order_index"],
    notes: optionalJsonText(json["notes"]),
    exercise: { id: exercise["id"], name: exercise["name"], category: exercise["category"] },
    sets,
  });
}

/** Parse an untrusted workout detail payload with embedded exercises and sets. */
export function parseWorkoutDetail(json: Json): Result<WorkoutDetail> {
  if (
    !isJsonObject(json) ||
    !isJsonNumber(json["id"]) ||
    !isJsonString(json["date"]) ||
    !isOptionalJsonNumber(json["body_weight_kg"])
  ) {
    return failure(WORKOUT_READ_ERROR);
  }
  if (!isJsonArray(json["exercises"])) {
    return failure(WORKOUT_READ_ERROR);
  }
  const exercises: WorkoutExerciseEntry[] = [];
  for (const raw of json["exercises"]) {
    const entry = parseWorkoutExerciseEntry(raw);
    if (!entry.ok) {
      return entry;
    }
    exercises.push(entry.value);
  }
  return success({
    id: json["id"],
    date: json["date"],
    name: optionalJsonText(json["name"]),
    notes: optionalJsonText(json["notes"]),
    bodyWeightKg: optionalJsonNumber(json["body_weight_kg"]),
    exercises,
  });
}

/** Parse an untrusted workout list response; any non-conforming entry rejects the payload. */
export function parseWorkoutSummaryList(json: Json): Result<readonly WorkoutSummary[]> {
  if (!isJsonArray(json)) {
    return failure(WORKOUT_LIST_READ_ERROR);
  }
  const workouts: WorkoutSummary[] = [];
  for (const raw of json) {
    if (!isJsonObject(raw) || !isJsonNumber(raw["id"]) || !isJsonString(raw["date"])) {
      return failure(WORKOUT_LIST_READ_ERROR);
    }
    workouts.push({
      id: raw["id"],
      date: raw["date"],
      name: optionalJsonText(raw["name"]),
    });
  }
  return success(workouts);
}

/**
 * Validate a workout log form on the client before any request is sent: a
 * padded `YYYY-MM-DD` date, at least one exercise, and every set carrying
 * reps or duration (at least one) with RPE in 1-10.
 */
export function parseWorkoutInput(
  date: string,
  name: string,
  notes: string,
  bodyWeightKg: string,
  exercises: readonly WorkoutExerciseInput[],
): Result<WorkoutInput> {
  const padded = padDate(date);
  if (padded === null) {
    return failure("Enter a valid date in YYYY-MM-DD format.");
  }
  const weight = bodyWeightKg.trim();
  let parsedWeight: number | null = null;
  if (weight.length > 0) {
    parsedWeight = Number(weight);
    if (!Number.isFinite(parsedWeight) || parsedWeight <= 0) {
      return failure("Enter a valid body weight in kg.");
    }
  }
  if (exercises.length === 0) {
    return failure("Add at least one exercise to the workout.");
  }
  for (const entry of exercises) {
    if (entry.sets.length === 0) {
      return failure(`Add at least one set to exercise ${entry.exerciseId}.`);
    }
    for (const set of entry.sets) {
      const label = `Set ${set.setNumber}`;
      if (set.reps === null && set.durationSeconds === null) {
        return failure(`${label} needs reps or a duration in seconds.`);
      }
      if (set.reps !== null && (!Number.isFinite(set.reps) || set.reps <= 0)) {
        return failure(`${label} reps must be a positive number.`);
      }
      if (
        set.durationSeconds !== null &&
        (!Number.isFinite(set.durationSeconds) || set.durationSeconds <= 0)
      ) {
        return failure(`${label} duration must be a positive number of seconds.`);
      }
      if (set.weightKg !== null && (!Number.isFinite(set.weightKg) || set.weightKg < 0)) {
        return failure(`${label} added weight must be a positive number of kg.`);
      }
      if (set.rpe !== null && (!Number.isFinite(set.rpe) || set.rpe < 1 || set.rpe > 10)) {
        return failure(`${label} RPE must be between 1 and 10.`);
      }
    }
  }
  return success({
    date: padded,
    name: trimmedOrNull(name),
    notes: trimmedOrNull(notes),
    bodyWeightKg: parsedWeight,
    exercises,
  });
}

/** Normalize a `YYYY-M-D` or `YYYY-MM-DD` date to the padded wire format, or null. */
function padDate(value: string): string | null {
  const match = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(value.trim());
  if (match === null) {
    return null;
  }
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const parsed = new Date(Date.UTC(year, month - 1, day));
  if (
    parsed.getUTCFullYear() !== year ||
    parsed.getUTCMonth() !== month - 1 ||
    parsed.getUTCDate() !== day
  ) {
    return null;
  }
  return `${match[1]}-${match[2]?.padStart(2, "0")}-${match[3]?.padStart(2, "0")}`;
}

/** Trim optional JSON numbers, mapping absent values to null. */
function optionalJsonNumber(value: Json | undefined): number | null {
  return isJsonNumber(value) ? value : null;
}

/** Decide whether a JSON value is absent, null, or a finite number. */
function isOptionalJsonNumber(value: Json | undefined): boolean {
  return value === undefined || value === null || isJsonNumber(value);
}

/** Decide whether a JSON value is absent, null, or a string. */
function isOptionalJsonString(value: Json | undefined): boolean {
  return value === undefined || value === null || isJsonString(value);
}

function trimmedOrNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

/** One aggregated progress point, one per workout date the exercise appears on. */
export type ProgressPoint = {
  readonly date: string;
  readonly bestReps: number | null;
  readonly bestWeightKg: number | null;
  readonly totalVolumeKg: number | null;
  readonly bestRpe: number | null;
  readonly estimated1RmKg: number | null;
};

/** One exercise's aggregated history as served by the progress endpoint. */
export type ExerciseProgress = {
  readonly exercise: Exercise;
  readonly data: readonly ProgressPoint[];
};

/** One body weight log entry as served by the Axum API. */
export type BodyWeightEntry = {
  readonly id: number;
  readonly date: string;
  readonly weightKg: number;
};

/** A new body weight entry as entered in the log form, ready for the Axum API. */
export type BodyWeightInput = {
  readonly date: string;
  readonly weightKg: number;
};

/** The single user profile row; fields are null until configured. */
export type Profile = {
  readonly heightCm: number | null;
  readonly sex: string | null;
};

/**
 * A profile update as entered in the profile form, ready for the Axum API.
 * Absent fields are null so the API's partial update keeps the current values.
 */
export type ProfileInput = {
  readonly heightCm: number | null;
  readonly sex: string | null;
};

/**
 * Parse an untrusted progress payload:
 * `{ exercise, data: [{ date, best_reps, best_weight_kg, total_volume_kg, best_rpe, estimated_1rm_kg }] }`
 * with the metrics optional (null when the exercise had no such sets that day).
 */
export function parseExerciseProgress(json: Json): Result<ExerciseProgress> {
  if (!isJsonObject(json) || !isJsonArray(json["data"])) {
    return failure(PROGRESS_READ_ERROR);
  }
  const exercise = parseExercise(json["exercise"]);
  if (!exercise.ok) {
    return failure(PROGRESS_READ_ERROR);
  }
  const data: ProgressPoint[] = [];
  for (const raw of json["data"]) {
    if (
      !isJsonObject(raw) ||
      !isJsonString(raw["date"]) ||
      !isOptionalJsonNumber(raw["best_reps"]) ||
      !isOptionalJsonNumber(raw["best_weight_kg"]) ||
      !isOptionalJsonNumber(raw["total_volume_kg"]) ||
      !isOptionalJsonNumber(raw["best_rpe"]) ||
      !isOptionalJsonNumber(raw["estimated_1rm_kg"])
    ) {
      return failure(PROGRESS_READ_ERROR);
    }
    data.push({
      date: raw["date"],
      bestReps: optionalJsonNumber(raw["best_reps"]),
      bestWeightKg: optionalJsonNumber(raw["best_weight_kg"]),
      totalVolumeKg: optionalJsonNumber(raw["total_volume_kg"]),
      bestRpe: optionalJsonNumber(raw["best_rpe"]),
      estimated1RmKg: optionalJsonNumber(raw["estimated_1rm_kg"]),
    });
  }
  return success({ exercise: exercise.value, data });
}

/**
 * Parse an untrusted body weight payload as served by the Axum API:
 * `{ id, date, weight_kg }` with every field present.
 */
export function parseBodyWeightEntry(json: Json): Result<BodyWeightEntry> {
  if (
    !isJsonObject(json) ||
    !isJsonNumber(json["id"]) ||
    !isJsonString(json["date"]) ||
    !isJsonNumber(json["weight_kg"])
  ) {
    return failure(BODY_WEIGHT_READ_ERROR);
  }
  return success({
    id: json["id"],
    date: json["date"],
    weightKg: json["weight_kg"],
  });
}

/** Parse an untrusted body weight list response; any non-conforming entry rejects the payload. */
export function parseBodyWeightList(json: Json): Result<readonly BodyWeightEntry[]> {
  if (!isJsonArray(json)) {
    return failure(BODY_WEIGHT_READ_ERROR);
  }
  const entries: BodyWeightEntry[] = [];
  for (const raw of json) {
    const entry = parseBodyWeightEntry(raw);
    if (!entry.ok) {
      return failure(BODY_WEIGHT_READ_ERROR);
    }
    entries.push(entry.value);
  }
  return success(entries);
}

/** Validate a body weight log form on the client before any request is sent. */
export function parseBodyWeightInput(date: string, weightKg: string): Result<BodyWeightInput> {
  const padded = padDate(date);
  if (padded === null) {
    return failure("Enter a valid date in YYYY-MM-DD format.");
  }
  const weight = weightKg.trim();
  if (weight.length === 0) {
    return failure("Enter a body weight in kg.");
  }
  const parsedWeight = Number(weight);
  if (!Number.isFinite(parsedWeight) || parsedWeight <= 0) {
    return failure("Enter a valid body weight in kg.");
  }
  return success({ date: padded, weightKg: parsedWeight });
}

/**
 * Parse an untrusted profile payload as served by the Axum API:
 * `{ height_cm, sex }`, both null when the profile is unconfigured.
 */
export function parseProfile(json: Json): Result<Profile> {
  if (
    !isJsonObject(json) ||
    !isOptionalJsonNumber(json["height_cm"]) ||
    !isOptionalJsonString(json["sex"])
  ) {
    return failure(PROFILE_READ_ERROR);
  }
  return success({
    heightCm: optionalJsonNumber(json["height_cm"]),
    sex: optionalJsonText(json["sex"]),
  });
}

/**
 * Validate the profile form on the client before any request is sent. Blank
 * fields map to null so the API's partial update keeps the current values.
 */
export function parseProfileInput(heightCm: string, sex: string): Result<ProfileInput> {
  const height = heightCm.trim();
  let parsedHeight: number | null = null;
  if (height.length > 0) {
    parsedHeight = Number(height);
    if (!Number.isFinite(parsedHeight) || parsedHeight <= 0) {
      return failure("Enter a valid height in cm.");
    }
  }
  return success({
    heightCm: parsedHeight,
    sex: trimmedOrNull(sex),
  });
}
