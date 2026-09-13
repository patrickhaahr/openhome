import { describe, expect, it } from "vitest";

import { failure } from "./result";
import {
  exerciseCategories,
  filterExercises,
  parseExercise,
  parseExerciseInput,
  parseExerciseList,
  type Exercise,
} from "./fitness";

function exercise(id: number, name: string, category = "gym", muscleGroup: string | null = "Chest"): Exercise {
  return { id, name, category, muscleGroup, equipment: null };
}

describe("parseExercise", () => {
  it("accepts a full exercise payload with snake_case optional fields", () => {
    expect(
      parseExercise({
        id: 1,
        name: "Bench Press",
        category: "gym",
        muscle_group: "Chest",
        equipment: "Barbell",
      }),
    ).toEqual({
      ok: true,
      value: { id: 1, name: "Bench Press", category: "gym", muscleGroup: "Chest", equipment: "Barbell" },
    });
  });

  it("maps absent or blank optional fields to null", () => {
    expect(parseExercise({ id: 2, name: "Push-up", category: "calisthenics" })).toEqual({
      ok: true,
      value: { id: 2, name: "Push-up", category: "calisthenics", muscleGroup: null, equipment: null },
    });
    expect(
      parseExercise({ id: 3, name: "Pull-up", category: "calisthenics", muscle_group: "  ", equipment: null }),
    ).toEqual({
      ok: true,
      value: { id: 3, name: "Pull-up", category: "calisthenics", muscleGroup: null, equipment: null },
    });
  });

  it("rejects payloads missing required fields or with wrong types", () => {
    const error = "Couldn't read the exercise from the Axum API.";
    expect(parseExercise(null)).toEqual(failure(error));
    expect(parseExercise("exercise")).toEqual(failure(error));
    expect(parseExercise({ name: "Bench Press", category: "gym" })).toEqual(failure(error));
    expect(parseExercise({ id: "1", name: "Bench Press", category: "gym" })).toEqual(failure(error));
    expect(parseExercise({ id: 1, name: 42, category: "gym" })).toEqual(failure(error));
    expect(parseExercise({ id: 1, name: "Bench Press" })).toEqual(failure(error));
  });
});

describe("parseExerciseList", () => {
  it("accepts an array of exercise payloads", () => {
    const payload = [
      { id: 1, name: "Push-up", category: "calisthenics", muscle_group: "Chest", equipment: null },
      { id: 2, name: "Squat", category: "gym", muscle_group: null, equipment: "Rack" },
    ];
    expect(parseExerciseList(payload)).toEqual({
      ok: true,
      value: [
        { id: 1, name: "Push-up", category: "calisthenics", muscleGroup: "Chest", equipment: null },
        { id: 2, name: "Squat", category: "gym", muscleGroup: null, equipment: "Rack" },
      ],
    });
  });

  it("rejects a non-array payload or one non-conforming entry", () => {
    const error = "Couldn't read the exercise library from the Axum API.";
    expect(parseExerciseList({ exercises: [] })).toEqual(failure(error));
    expect(parseExerciseList([{ id: 1, name: "Push-up" }])).toEqual(failure(error));
  });
});

describe("parseExerciseInput", () => {
  it("accepts a trimmed name with a known category and trims optional fields", () => {
    expect(parseExerciseInput("  Pull-up  ", "calisthenics", " Back ", " Bar ")).toEqual({
      ok: true,
      value: { name: "Pull-up", category: "calisthenics", muscleGroup: "Back", equipment: "Bar" },
    });
  });

  it("maps blank optional fields to null", () => {
    expect(parseExerciseInput("Pull-up", "calisthenics", "", "   ")).toEqual({
      ok: true,
      value: { name: "Pull-up", category: "calisthenics", muscleGroup: null, equipment: null },
    });
  });

  it("rejects a blank name or an unknown category before any request is sent", () => {
    expect(parseExerciseInput("   ", "gym", "", "")).toEqual(
      failure("Enter a name for the exercise."),
    );
    expect(parseExerciseInput("Bench Press", "cardio", "", "")).toEqual(
      failure("Choose calisthenics or gym as the category."),
    );
  });

  it("only accepts the categories the Axum API validates", () => {
    expect(exerciseCategories).toEqual(["calisthenics", "gym"]);
  });
});

describe("filterExercises", () => {
  const library = [
    exercise(1, "Push-up", "calisthenics", "Chest"),
    exercise(2, "Bench Press", "gym", "Chest"),
    exercise(3, "Pull-up", "calisthenics", "Back"),
    exercise(4, "Plank", "calisthenics", null),
  ];

  it("returns every exercise when no filter is set", () => {
    expect(filterExercises(library, "", "all", "")).toEqual(library);
  });

  it("searches names case-insensitively", () => {
    expect(filterExercises(library, "PULL", "all", "").map((entry) => entry.id)).toEqual([3]);
  });

  it("filters by category", () => {
    expect(filterExercises(library, "", "calisthenics", "").map((entry) => entry.id)).toEqual([1, 3, 4]);
  });

  it("filters by muscle group case-insensitively and excludes unclassified exercises", () => {
    expect(filterExercises(library, "", "all", "chest").map((entry) => entry.id)).toEqual([1, 2]);
    expect(filterExercises(library, "", "all", "core")).toEqual([]);
  });

  it("combines search and filters", () => {
    expect(filterExercises(library, "press", "gym", "chest").map((entry) => entry.id)).toEqual([2]);
    expect(filterExercises(library, "press", "calisthenics", "chest")).toEqual([]);
  });
});
