import { describe, expect, it } from "vitest";

import type { WorkoutDetail, WorkoutInput, WorkoutSummary } from "../domain/fitness";
import { failure, success, type Result } from "../domain/result";
import {
  createWorkoutsController,
  initialState,
  reduce,
  type WorkoutsEvent,
  type WorkoutsState,
} from "./use-workouts";

function workoutDetail(id: number, date: string): WorkoutDetail {
  return { id, date, name: null, notes: null, bodyWeightKg: null, exercises: [] };
}

function workoutInput(date: string): WorkoutInput {
  return { date, name: null, notes: null, bodyWeightKg: null, exercises: [] };
}

/** A scripted fake of the workout adapter surface with manually resolved responses. */
function fakeApi() {
  const calls: Array<
    | { readonly kind: "listWorkouts" }
    | { readonly kind: "getWorkout"; readonly id: number }
    | { readonly kind: "createWorkout"; readonly input: WorkoutInput }
    | { readonly kind: "updateWorkout"; readonly id: number; readonly input: WorkoutInput }
    | { readonly kind: "deleteWorkout"; readonly id: number }
  > = [];
  const listPending: Array<(result: Result<readonly WorkoutSummary[]>) => void> = [];
  const getPending: Array<(result: Result<WorkoutDetail>) => void> = [];
  const savePending: Array<(result: Result<WorkoutDetail>) => void> = [];
  const deletePending: Array<(result: Result<void>) => void> = [];
  const api = {
    listWorkouts: () => {
      calls.push({ kind: "listWorkouts" });
      return new Promise<Result<readonly WorkoutSummary[]>>((resolve) => {
        listPending.push(resolve);
      });
    },
    getWorkout: (id: number) => {
      calls.push({ kind: "getWorkout", id });
      return new Promise<Result<WorkoutDetail>>((resolve) => {
        getPending.push(resolve);
      });
    },
    createWorkout: (input: WorkoutInput) => {
      calls.push({ kind: "createWorkout", input });
      return new Promise<Result<WorkoutDetail>>((resolve) => {
        savePending.push(resolve);
      });
    },
    updateWorkout: (id: number, input: WorkoutInput) => {
      calls.push({ kind: "updateWorkout", id, input });
      return new Promise<Result<WorkoutDetail>>((resolve) => {
        savePending.push(resolve);
      });
    },
    deleteWorkout: (id: number) => {
      calls.push({ kind: "deleteWorkout", id });
      return new Promise<Result<void>>((resolve) => {
        deletePending.push(resolve);
      });
    },
  };
  return { api, calls, listPending, getPending, savePending, deletePending };
}

function harness() {
  const fake = fakeApi();
  const events: WorkoutsEvent[] = [];
  const controller = createWorkoutsController({
    api: fake.api,
    emit: (event) => events.push(event),
  });
  return { ...fake, controller, events };
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function replay(events: ReadonlyArray<WorkoutsEvent>): WorkoutsState {
  return events.reduce((state, event) => reduce(state, event), initialState);
}

const HISTORY: readonly WorkoutSummary[] = [
  { id: 9, date: "2026-09-12", name: "Push A" },
  { id: 8, date: "2026-09-10", name: null },
];

describe("workouts state machine", () => {
  it("loads history into the ready state", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(HISTORY));
    await settle();

    const state = replay(h.events);
    expect(state.workouts).toEqual(HISTORY);
    expect(state.refreshing).toBe(false);
    expect(state.error).toBeNull();
  });

  it("surfaces a failed history load as an error without losing the list", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(HISTORY));
    await settle();

    h.controller.refresh();
    h.listPending[1]?.(failure("Couldn't reach the Axum API."));
    await settle();

    const state = replay(h.events);
    expect(state.error).toBe("Couldn't reach the Axum API.");
    expect(state.workouts).toEqual(HISTORY);
    expect(state.refreshing).toBe(false);
  });

  it("opens a workout detail by id", async () => {
    const h = harness();
    h.controller.openWorkout(9);
    h.getPending[0]?.(success(workoutDetail(9, "2026-09-12")));
    await settle();

    const state = replay(h.events);
    expect(state.detail).toEqual({ tag: "loaded", workout: workoutDetail(9, "2026-09-12") });
  });

  it("surfaces a missing workout detail as an error", async () => {
    const h = harness();
    h.controller.openWorkout(404);
    h.getPending[0]?.(failure("Workout 404 not found."));
    await settle();

    expect(replay(h.events).detail).toEqual({ tag: "error", message: "Workout 404 not found." });
  });

  it("closes the detail view", async () => {
    const h = harness();
    h.controller.openWorkout(9);
    h.getPending[0]?.(success(workoutDetail(9, "2026-09-12")));
    await settle();
    h.controller.closeWorkout();

    expect(replay(h.events).detail.tag).toBe("closed");
  });

  it("saves a new workout and reloads history", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(HISTORY));
    await settle();

    h.controller.saveWorkout({ tag: "create", input: workoutInput("2026-09-12") });
    h.savePending[0]?.(success(workoutDetail(10, "2026-09-12")));
    await settle();
    h.listPending[1]?.(success([{ id: 10, date: "2026-09-12", name: null }, ...HISTORY]));
    await settle();

    const state = replay(h.events);
    expect(state.saving).toBe(false);
    expect(state.detail.tag).toBe("closed");
    expect(state.workouts.map((entry) => entry.id)).toEqual([10, 9, 8]);
    expect(h.calls.filter((call) => call.kind === "createWorkout")).toHaveLength(1);
  });

  it("updates an existing workout and reloads history", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(HISTORY));
    await settle();

    h.controller.saveWorkout({ tag: "update", id: 9, input: workoutInput("2026-09-13") });
    h.savePending[0]?.(success(workoutDetail(9, "2026-09-13")));
    await settle();
    h.listPending[1]?.(success(HISTORY));
    await settle();

    expect(h.calls.filter((call) => call.kind === "updateWorkout")).toEqual([
      { kind: "updateWorkout", id: 9, input: workoutInput("2026-09-13") },
    ]);
    expect(replay(h.events).saving).toBe(false);
  });

  it("keeps the detail closed logic away from a failed save and surfaces the error", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(HISTORY));
    await settle();

    h.controller.saveWorkout({ tag: "create", input: workoutInput("2026-09-12") });
    h.savePending[0]?.(failure("Set 1 needs reps or a duration in seconds."));
    await settle();

    const state = replay(h.events);
    expect(state.saving).toBe(false);
    expect(state.error).toBe("Set 1 needs reps or a duration in seconds.");
    expect(state.detail.tag).toBe("closed");
    expect(state.workouts.map((entry) => entry.id)).toEqual([9, 8]);
  });

  it("deletes a workout and reloads history", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(HISTORY));
    await settle();

    h.controller.openWorkout(9);
    h.getPending[0]?.(success(workoutDetail(9, "2026-09-12")));
    await settle();

    h.controller.deleteWorkout(9);
    h.deletePending[0]?.(success(undefined));
    await settle();
    h.listPending[1]?.(success([HISTORY[1] ?? HISTORY[0]!]));
    await settle();

    expect(h.calls.filter((call) => call.kind === "deleteWorkout")).toEqual([
      { kind: "deleteWorkout", id: 9 },
    ]);
    const state = replay(h.events);
    expect(state.workouts.map((entry) => entry.id)).toEqual([8]);
    expect(state.detail.tag).toBe("closed");
  });

  it("surfaces a failed delete without touching history", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(HISTORY));
    await settle();

    h.controller.deleteWorkout(9);
    h.deletePending[0]?.(failure("Workout 9 not found."));
    await settle();

    const state = replay(h.events);
    expect(state.error).toBe("Workout 9 not found.");
    expect(state.workouts.map((entry) => entry.id)).toEqual([9, 8]);
  });

  it("keeps the history refresh resolving when a detail opens mid-refresh", async () => {
    const h = harness();
    h.controller.refresh();
    h.controller.openWorkout(9);
    h.getPending[0]?.(success(workoutDetail(9, "2026-09-12")));
    h.listPending[0]?.(success(HISTORY));
    await settle();

    const state = replay(h.events);
    expect(state.refreshing).toBe(false);
    expect(state.workouts).toEqual(HISTORY);
    expect(state.detail.tag).toBe("loaded");
  });

  it("ignores a mutation while another is in flight", async () => {
    const h = harness();
    h.controller.deleteWorkout(9);
    h.controller.deleteWorkout(8);
    h.deletePending[0]?.(success(undefined));
    await settle();

    expect(h.calls.filter((call) => call.kind === "deleteWorkout")).toHaveLength(1);
  });
});
