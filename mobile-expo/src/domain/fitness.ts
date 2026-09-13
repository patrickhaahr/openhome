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

/**
 * Parse an untrusted exercise payload. The Axum API serves exercises as
 * `{ id, name, category, muscle_group, equipment }` with optional fields null.
 */
export function parseExercise(json: Json): Result<Exercise> {
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
    muscleGroup: optionalInputText(muscleGroup),
    equipment: optionalInputText(equipment),
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

/** Trim optional form text, mapping blank values to null. */
function optionalInputText(value: string): string | null {
  return trimmedOrNull(value);
}

function trimmedOrNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}
