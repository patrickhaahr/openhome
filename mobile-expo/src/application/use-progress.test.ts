import { describe, expect, it } from "vitest";

import type { ExerciseProgress } from "../domain/fitness";
import { failure, success, type Result } from "../domain/result";
import {
  createProgressController,
  initialState,
  reduce,
  type ProgressEvent,
  type ProgressState,
} from "./use-progress";

const PROGRESS: ExerciseProgress = {
  exercise: { id: 1, name: "Bench Press", category: "gym", muscleGroup: null, equipment: null },
  data: [
    {
      date: "2026-09-12",
      bestReps: 8,
      bestWeightKg: 60,
      totalVolumeKg: 480,
      bestRpe: 7,
      estimated1RmKg: 76,
    },
  ],
};

/** A scripted fake of the progress adapter surface with manually resolved responses. */
function harness() {
  const calls: Array<{ readonly kind: "getExerciseProgress"; readonly id: number }> = [];
  const pending: Array<(result: Result<ExerciseProgress>) => void> = [];
  const api = {
    getExerciseProgress: (id: number) => {
      calls.push({ kind: "getExerciseProgress", id });
      return new Promise<Result<ExerciseProgress>>((resolve) => {
        pending.push(resolve);
      });
    },
  };
  const events: ProgressEvent[] = [];
  const controller = createProgressController({
    api,
    emit: (event) => events.push(event),
  });
  return { api, calls, pending, controller, events };
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function replay(events: ReadonlyArray<ProgressEvent>): ProgressState {
  return events.reduce((state, event) => reduce(state, event), initialState);
}

describe("progress state machine", () => {
  it("loads a per-exercise progress series", async () => {
    const h = harness();
    h.controller.openExercise(1);
    h.pending[0]?.(success(PROGRESS));
    await settle();

    const state = replay(h.events);
    expect(state.tag).toBe("loaded");
    if (state.tag === "loaded") {
      expect(state.progress).toEqual(PROGRESS);
    }
    expect(h.calls).toEqual([{ kind: "getExerciseProgress", id: 1 }]);
  });

  it("surfaces a failed load as an error", async () => {
    const h = harness();
    h.controller.openExercise(404);
    h.pending[0]?.(failure("Exercise 404 not found."));
    await settle();

    expect(replay(h.events)).toEqual({
      tag: "error",
      exerciseId: 404,
      message: "Exercise 404 not found.",
    });
  });

  it("re-fetches the currently selected exercise on refresh", async () => {
    const h = harness();
    h.controller.openExercise(1);
    h.pending[0]?.(success(PROGRESS));
    await settle();

    h.controller.refresh();
    h.pending[1]?.(success(PROGRESS));
    await settle();

    expect(h.calls.map((call) => call.id)).toEqual([1, 1]);
    expect(replay(h.events).tag).toBe("loaded");
  });

  it("ignores a stale response for a superseded selection", async () => {
    const h = harness();
    h.controller.openExercise(1);
    h.controller.openExercise(2);
    h.pending[0]?.(success(PROGRESS));
    h.pending[1]?.(failure("Exercise 2 not found."));
    await settle();

    expect(replay(h.events).tag).toBe("error");
    expect(h.calls.map((call) => call.id)).toEqual([1, 2]);
  });

  it("does nothing on refresh with no exercise selected", () => {
    const h = harness();
    h.controller.refresh();
    expect(h.calls).toHaveLength(0);
    expect(h.events).toHaveLength(0);
  });
});
