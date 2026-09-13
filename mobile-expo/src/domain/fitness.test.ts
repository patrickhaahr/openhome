import { describe, expect, it } from "vitest";

import { failure } from "./result";
import {
  BODY_WEIGHT_READ_ERROR,
  exerciseCategories,
  filterExercises,
  parseBodyWeightEntry,
  parseBodyWeightInput,
  parseBodyWeightList,
  parseExercise,
  parseExerciseInput,
  parseExerciseList,
  parseExerciseProgress,
  parseProfile,
  parseProfileInput,
  parseWorkoutDetail,
  parseWorkoutInput,
  parseWorkoutSummaryList,
  PROFILE_READ_ERROR,
  PROGRESS_READ_ERROR,
  WORKOUT_READ_ERROR,
  type Exercise,
  type SetInput,
  type WorkoutExerciseInput,
} from "./fitness";

function exercise(
  id: number,
  name: string,
  category = "gym",
  muscleGroup: string | null = "Chest",
): Exercise {
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
      value: {
        id: 1,
        name: "Bench Press",
        category: "gym",
        muscleGroup: "Chest",
        equipment: "Barbell",
      },
    });
  });

  it("maps absent or blank optional fields to null", () => {
    expect(parseExercise({ id: 2, name: "Push-up", category: "calisthenics" })).toEqual({
      ok: true,
      value: {
        id: 2,
        name: "Push-up",
        category: "calisthenics",
        muscleGroup: null,
        equipment: null,
      },
    });
    expect(
      parseExercise({
        id: 3,
        name: "Pull-up",
        category: "calisthenics",
        muscle_group: "  ",
        equipment: null,
      }),
    ).toEqual({
      ok: true,
      value: {
        id: 3,
        name: "Pull-up",
        category: "calisthenics",
        muscleGroup: null,
        equipment: null,
      },
    });
  });

  it("rejects payloads missing required fields or with wrong types", () => {
    const error = "Couldn't read the exercise from the Axum API.";
    expect(parseExercise(null)).toEqual(failure(error));
    expect(parseExercise("exercise")).toEqual(failure(error));
    expect(parseExercise({ name: "Bench Press", category: "gym" })).toEqual(failure(error));
    expect(parseExercise({ id: "1", name: "Bench Press", category: "gym" })).toEqual(
      failure(error),
    );
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
    expect(filterExercises(library, "", "calisthenics", "").map((entry) => entry.id)).toEqual([
      1, 3, 4,
    ]);
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

function set(overrides: Partial<SetInput> = {}): SetInput {
  return {
    setNumber: 1,
    reps: 8,
    weightKg: null,
    durationSeconds: null,
    rpe: 7,
    notes: null,
    ...overrides,
  };
}

function workoutExercise(overrides: Partial<WorkoutExerciseInput> = {}): WorkoutExerciseInput {
  return { exerciseId: 1, notes: null, sets: [set()], ...overrides };
}

const workoutDetailPayload = {
  id: 5,
  date: "2026-09-12",
  name: "Push A",
  notes: "Felt strong",
  body_weight_kg: 72.5,
  exercises: [
    {
      id: 11,
      order_index: 0,
      notes: null,
      exercise: { id: 1, name: "Bench Press", category: "gym" },
      sets: [
        {
          id: 21,
          set_number: 1,
          reps: 8,
          weight_kg: 60,
          duration_seconds: null,
          rpe: 7,
          notes: null,
        },
      ],
    },
    {
      id: 12,
      order_index: 1,
      notes: "Rings",
      exercise: { id: 2, name: "Pull-up", category: "calisthenics" },
      sets: [
        {
          id: 22,
          set_number: 1,
          reps: 10,
          weight_kg: null,
          duration_seconds: null,
          rpe: 8,
          notes: null,
        },
        {
          id: 23,
          set_number: 2,
          reps: null,
          weight_kg: null,
          duration_seconds: 45,
          rpe: null,
          notes: "Hold",
        },
      ],
    },
  ],
};

describe("parseWorkoutSummaryList", () => {
  it("accepts a newest-first list of workout summaries", () => {
    expect(
      parseWorkoutSummaryList([
        { id: 9, date: "2026-09-12", name: "Push A" },
        { id: 8, date: "2026-09-10", name: null },
      ]),
    ).toEqual({
      ok: true,
      value: [
        { id: 9, date: "2026-09-12", name: "Push A" },
        { id: 8, date: "2026-09-10", name: null },
      ],
    });
  });

  it("rejects a non-array or non-conforming payload", () => {
    const error = "Couldn't read the workouts from the Axum API.";
    expect(parseWorkoutSummaryList({ workouts: [] })).toEqual(failure(error));
    expect(parseWorkoutSummaryList([{ id: 9, date: 20260912, name: null }])).toEqual(
      failure(error),
    );
  });
});

describe("parseWorkoutDetail", () => {
  it("parses the embedded exercises in order with snake_case sets", () => {
    const result = parseWorkoutDetail(workoutDetailPayload);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.exercises.map((entry) => entry.exercise.name)).toEqual([
        "Bench Press",
        "Pull-up",
      ]);
      expect(result.value.exercises[1]?.sets[1]).toEqual({
        id: 23,
        setNumber: 2,
        reps: null,
        weightKg: null,
        durationSeconds: 45,
        rpe: null,
        notes: "Hold",
      });
    }
  });

  it("rejects a payload with a non-conforming exercise or set", () => {
    expect(parseWorkoutDetail({ id: 5, date: "2026-09-12", exercises: [{}] })).toEqual(
      failure(WORKOUT_READ_ERROR),
    );
    expect(
      parseWorkoutDetail({
        ...workoutDetailPayload,
        exercises: [
          {
            id: 11,
            order_index: 0,
            exercise: { id: 1, name: "Bench Press", category: "gym" },
            sets: [
              {
                id: 21,
                set_number: 1,
                reps: "eight",
                weight_kg: null,
                duration_seconds: null,
                rpe: null,
                notes: null,
              },
            ],
          },
        ],
      }),
    ).toEqual(failure(WORKOUT_READ_ERROR));
  });
});

describe("parseWorkoutInput", () => {
  it("accepts a mixed workout with padded date, trimmed text, and parsed numbers", () => {
    expect(
      parseWorkoutInput("2026-9-2", "  Morning calisthenics  ", " Felt good ", "72.5", [
        workoutExercise({ exerciseId: 1 }),
        workoutExercise({ exerciseId: 2 }),
      ]),
    ).toEqual({
      ok: true,
      value: {
        date: "2026-09-02",
        name: "Morning calisthenics",
        notes: "Felt good",
        bodyWeightKg: 72.5,
        exercises: [
          { exerciseId: 1, notes: null, sets: [set()] },
          { exerciseId: 2, notes: null, sets: [set()] },
        ],
      },
    });
  });

  it("maps blank name, notes, and body weight to null", () => {
    const result = parseWorkoutInput("2026-09-02", "  ", "", "", [workoutExercise()]);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.name).toBeNull();
      expect(result.value.notes).toBeNull();
      expect(result.value.bodyWeightKg).toBeNull();
    }
  });

  it("rejects an invalid date or body weight", () => {
    expect(parseWorkoutInput("not-a-date", "", "", "", [workoutExercise()])).toEqual(
      failure("Enter a valid date in YYYY-MM-DD format."),
    );
    expect(parseWorkoutInput("2026-09-02", "", "", "heavy", [workoutExercise()])).toEqual(
      failure("Enter a valid body weight in kg."),
    );
  });

  it("rejects a workout with no exercises", () => {
    expect(parseWorkoutInput("2026-09-02", "", "", "", [])).toEqual(
      failure("Add at least one exercise to the workout."),
    );
  });

  it("rejects a set with neither reps nor duration", () => {
    const result = parseWorkoutInput("2026-09-02", "", "", "", [
      workoutExercise({ sets: [set({ reps: null, durationSeconds: null })] }),
    ]);
    expect(result).toEqual(failure("Set 1 needs reps or a duration in seconds."));
  });

  it("rejects RPE outside 1-10", () => {
    const result = parseWorkoutInput("2026-09-02", "", "", "", [
      workoutExercise({ sets: [set({ rpe: 11 })] }),
    ]);
    expect(result).toEqual(failure("Set 1 RPE must be between 1 and 10."));
  });

  it("rejects a negative weight or duration", () => {
    expect(
      parseWorkoutInput("2026-09-02", "", "", "", [
        workoutExercise({ sets: [set({ weightKg: -1 })] }),
      ]),
    ).toEqual(failure("Set 1 added weight must be a positive number of kg."));
    expect(
      parseWorkoutInput("2026-09-02", "", "", "", [
        workoutExercise({ sets: [set({ durationSeconds: 0 })] }),
      ]),
    ).toEqual(failure("Set 1 duration must be a positive number of seconds."));
  });
});

describe("parseExerciseProgress", () => {
  it("accepts a progress payload with the embedded exercise and points", () => {
    expect(
      parseExerciseProgress({
        exercise: { id: 1, name: "Bench Press", category: "gym", muscle_group: "Chest", equipment: null },
        data: [
          {
            date: "2026-09-12",
            best_reps: 8,
            best_weight_kg: 60,
            total_volume_kg: 480,
            best_rpe: 7,
            estimated_1rm_kg: 76,
          },
          {
            date: "2026-09-14",
            best_reps: null,
            best_weight_kg: null,
            total_volume_kg: null,
            best_rpe: null,
            estimated_1rm_kg: null,
          },
        ],
      }),
    ).toEqual({
      ok: true,
      value: {
        exercise: exercise(1, "Bench Press"),
        data: [
          {
            date: "2026-09-12",
            bestReps: 8,
            bestWeightKg: 60,
            totalVolumeKg: 480,
            bestRpe: 7,
            estimated1RmKg: 76,
          },
          {
            date: "2026-09-14",
            bestReps: null,
            bestWeightKg: null,
            totalVolumeKg: null,
            bestRpe: null,
            estimated1RmKg: null,
          },
        ],
      },
    });
  });

  it("rejects a non-conforming payload", () => {
    expect(parseExerciseProgress(null)).toEqual(failure(PROGRESS_READ_ERROR));
    expect(parseExerciseProgress({ exercise: null, data: [] })).toEqual(
      failure(PROGRESS_READ_ERROR),
    );
    expect(
      parseExerciseProgress({
        exercise: exercise(1, "Bench Press"),
        data: [{ date: 12, best_reps: null }],
      }),
    ).toEqual(failure(PROGRESS_READ_ERROR));
    expect(
      parseExerciseProgress({
        exercise: exercise(1, "Bench Press"),
        data: [{ best_reps: 8 }],
      }),
    ).toEqual(failure(PROGRESS_READ_ERROR));
  });
});

describe("parseBodyWeightEntry", () => {
  it("accepts an entry with snake_case fields", () => {
    expect(parseBodyWeightEntry({ id: 3, date: "2026-09-12", weight_kg: 72.5 })).toEqual({
      ok: true,
      value: { id: 3, date: "2026-09-12", weightKg: 72.5 },
    });
  });

  it("rejects a non-conforming entry", () => {
    expect(parseBodyWeightEntry(null)).toEqual(failure(BODY_WEIGHT_READ_ERROR));
    expect(parseBodyWeightEntry({ id: 3, date: "2026-09-12" })).toEqual(
      failure(BODY_WEIGHT_READ_ERROR),
    );
    expect(parseBodyWeightEntry({ id: "3", date: "2026-09-12", weight_kg: 72.5 })).toEqual(
      failure(BODY_WEIGHT_READ_ERROR),
    );
  });

  it("rejects a list with any bad entry", () => {
    expect(parseBodyWeightList(null)).toEqual(failure(BODY_WEIGHT_READ_ERROR));
    expect(
      parseBodyWeightList([
        { id: 1, date: "2026-09-12", weight_kg: 72.5 },
        { id: 2, date: "2026-09-13" },
      ]),
    ).toEqual(failure(BODY_WEIGHT_READ_ERROR));
  });
});

describe("parseBodyWeightInput", () => {
  it("pads the date and parses the weight", () => {
    expect(parseBodyWeightInput("2026-9-2", " 72.5 ")).toEqual({
      ok: true,
      value: { date: "2026-09-02", weightKg: 72.5 },
    });
  });

  it("rejects an invalid date or non-positive weight", () => {
    expect(parseBodyWeightInput("nope", "72")).toEqual(
      failure("Enter a valid date in YYYY-MM-DD format."),
    );
    expect(parseBodyWeightInput("2026-09-02", "")).toEqual(failure("Enter a body weight in kg."));
    expect(parseBodyWeightInput("2026-09-02", "heavy")).toEqual(
      failure("Enter a valid body weight in kg."),
    );
    expect(parseBodyWeightInput("2026-09-02", "0")).toEqual(
      failure("Enter a valid body weight in kg."),
    );
  });
});

describe("parseProfile", () => {
  it("accepts a configured profile", () => {
    expect(parseProfile({ height_cm: 182.5, sex: "male" })).toEqual({
      ok: true,
      value: { heightCm: 182.5, sex: "male" },
    });
  });

  it("accepts the unconfigured profile with null fields", () => {
    expect(parseProfile({ height_cm: null, sex: null })).toEqual({
      ok: true,
      value: { heightCm: null, sex: null },
    });
  });

  it("rejects a non-conforming profile", () => {
    expect(parseProfile(null)).toEqual(failure(PROFILE_READ_ERROR));
    expect(parseProfile({ height_cm: "182" })).toEqual(failure(PROFILE_READ_ERROR));
    expect(parseProfile({ height_cm: 182, sex: 3 })).toEqual(failure(PROFILE_READ_ERROR));
  });
});

describe("parseProfileInput", () => {
  it("parses height and trims sex", () => {
    expect(parseProfileInput(" 182.5 ", " male ")).toEqual({
      ok: true,
      value: { heightCm: 182.5, sex: "male" },
    });
  });

  it("leaves untouched fields absent so the API keeps the current values", () => {
    expect(parseProfileInput(undefined, undefined)).toEqual({
      ok: true,
      value: { heightCm: undefined, sex: undefined },
    });
    expect(parseProfileInput(undefined, " male ")).toEqual({
      ok: true,
      value: { heightCm: undefined, sex: "male" },
    });
  });

  it("maps blank touched fields to explicit null so the API clears them", () => {
    expect(parseProfileInput("", "")).toEqual({
      ok: true,
      value: { heightCm: null, sex: null },
    });
    expect(parseProfileInput("182.5", "")).toEqual({
      ok: true,
      value: { heightCm: 182.5, sex: null },
    });
  });

  it("rejects an invalid height", () => {
    expect(parseProfileInput("tall", "")).toEqual(failure("Enter a valid height in cm."));
    expect(parseProfileInput("0", "")).toEqual(failure("Enter a valid height in cm."));
  });
});
