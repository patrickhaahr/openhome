import { describe, expect, it } from "vitest";

import type { Exercise, ExerciseInput } from "../domain/fitness";
import { failure, success, type Result } from "../domain/result";
import {
  createFitnessController,
  reduce,
  type FitnessEvent,
  type FitnessState,
} from "./use-fitness";

function exercise(id: number, name: string): Exercise {
  return { id, name, category: "gym", muscleGroup: null, equipment: null };
}

const input: ExerciseInput = { name: "Bench Press", category: "gym", muscleGroup: "Chest", equipment: "Barbell" };

/** A scripted fake of the fitness adapter surface with manually resolved responses. */
function fakeApi() {
  const calls: Array<{ readonly kind: "list" } | { readonly kind: "create"; readonly input: ExerciseInput }> = [];
  const listPending: Array<(result: Result<readonly Exercise[]>) => void> = [];
  const createPending: Array<(result: Result<Exercise>) => void> = [];
  const api = {
    listExercises: () => {
      calls.push({ kind: "list" });
      return new Promise<Result<readonly Exercise[]>>((resolve) => {
        listPending.push(resolve);
      });
    },
    createExercise: (submitted: ExerciseInput) => {
      calls.push({ kind: "create", input: submitted });
      return new Promise<Result<Exercise>>((resolve) => {
        createPending.push(resolve);
      });
    },
  };
  return { api, calls, listPending, createPending };
}

function harness() {
  const fake = fakeApi();
  const events: FitnessEvent[] = [];
  const controller = createFitnessController({ api: fake.api, emit: (event) => events.push(event) });
  return { ...fake, controller, events };
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function replay(events: ReadonlyArray<FitnessEvent>, initial: FitnessState): FitnessState {
  return events.reduce((state, event) => reduce(state, event), initial);
}

const initial: FitnessState = { tag: "loading" };
const LIBRARY = [exercise(1, "Push-up"), exercise(2, "Squat")];

describe("fitness state machine", () => {
  it("loads the exercise library into a ready list", async () => {
    const h = harness();
    h.controller.refresh();
    expect(h.calls).toEqual([{ kind: "list" }]);

    h.listPending[0]?.(success(LIBRARY));
    await settle();

    expect(h.events.map((event) => event.type)).toEqual(["loadStarted", "loadSucceeded"]);
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.exercises).toEqual(LIBRARY);
    expect(state.tag === "ready" && state.error).toBeNull();
  });

  it("surfaces a failed library load as a retryable error", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(failure("Couldn't reach the Axum API."));
    await settle();

    expect(replay(h.events, initial)).toEqual({
      tag: "error",
      message: "Couldn't reach the Axum API.",
    });

    h.controller.refresh();
    h.listPending[1]?.(success(LIBRARY));
    await settle();
    expect(replay(h.events, initial).tag).toBe("ready");
  });

  it("adds an exercise and reloads the library so it appears immediately", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(LIBRARY));
    await settle();

    h.controller.create(input);
    expect(h.calls).toEqual([{ kind: "list" }, { kind: "create", input }]);
    const created = exercise(3, "Bench Press");
    h.createPending[0]?.(success(created));
    await settle();
    expect(h.calls).toEqual([
      { kind: "list" },
      { kind: "create", input },
      { kind: "list" },
    ]);

    h.listPending[1]?.(success([...LIBRARY, created]));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.exercises.map((entry) => entry.id)).toEqual([1, 2, 3]);
    expect(state.tag === "ready" && state.busy).toBe(false);
  });

  it("surfaces a failed create without touching the list", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(LIBRARY));
    await settle();

    h.controller.create(input);
    h.createPending[0]?.(failure("Exercise 'Bench Press' already exists"));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.error).toBe("Exercise 'Bench Press' already exists");
    expect(state.tag === "ready" && state.busy).toBe(false);
    expect(state.tag === "ready" && state.exercises).toEqual(LIBRARY);
    expect(h.calls).toEqual([{ kind: "list" }, { kind: "create", input }]);
  });

  it("ignores a create while another mutation is in flight", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(LIBRARY));
    await settle();

    h.controller.create(input);
    h.controller.create(input);
    h.createPending[0]?.(success(exercise(3, "Bench Press")));
    await settle();

    expect(h.calls.filter((call) => call.kind === "create")).toHaveLength(1);
  });
});
