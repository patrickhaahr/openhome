import { useEffect, useMemo, useReducer, useRef } from "react";

import type { Exercise, ExerciseInput } from "../domain/fitness";
import type { FitnessApi } from "../infrastructure/open-home-api";

/** The user-visible state of the Fitness Tab's exercise library. */
export type FitnessState =
  | { readonly tag: "loading" }
  | { readonly tag: "error"; readonly message: string }
  | {
      readonly tag: "ready";
      readonly exercises: readonly Exercise[];
      /** A list refresh is in flight; the previous list stays visible. */
      readonly refreshing: boolean;
      /** A create is in flight. */
      readonly busy: boolean;
      readonly error: string | null;
    };

/** User actions accepted by the exercise library. */
export type FitnessActions = {
  readonly refresh: () => void;
  /** The input is validated on the client by the caller before this is invoked. */
  readonly create: (input: ExerciseInput) => void;
};

export type FitnessEvent =
  | { readonly type: "loadStarted" }
  | { readonly type: "loadSucceeded"; readonly exercises: readonly Exercise[] }
  | { readonly type: "loadFailed"; readonly message: string }
  | { readonly type: "createStarted" }
  | { readonly type: "createSucceeded" }
  | { readonly type: "createFailed"; readonly message: string }
  /** A cancelled controller's in-flight mutation resolved; its flag must clear. */
  | { readonly type: "superseded"; readonly of: "mutation" };

/** Drives the exercise library against the Axum API adapter, emitting events. */
export function createFitnessController(deps: {
  readonly api: Pick<FitnessApi, "listExercises" | "createExercise">;
  readonly emit: (event: FitnessEvent) => void;
}) {
  let loadToken = 0;
  let mutateToken = 0;
  let busy = false;

  async function load(): Promise<void> {
    const current = ++loadToken;
    deps.emit({ type: "loadStarted" });
    const result = await deps.api.listExercises();
    if (loadToken !== current) {
      return;
    }
    deps.emit(
      result.ok
        ? { type: "loadSucceeded", exercises: result.value }
        : { type: "loadFailed", message: result.error },
    );
  }

  async function create(input: ExerciseInput): Promise<void> {
    if (busy) {
      return;
    }
    busy = true;
    const current = ++mutateToken;
    // A load in flight since before this mutation carries a stale snapshot; drop it.
    loadToken += 1;
    deps.emit({ type: "createStarted" });
    const result = await deps.api.createExercise(input);
    if (mutateToken !== current) {
      busy = false;
      deps.emit({ type: "superseded", of: "mutation" });
      return;
    }
    busy = false;
    if (!result.ok) {
      deps.emit({ type: "createFailed", message: result.error });
      return;
    }
    deps.emit({ type: "createSucceeded" });
    void load();
  }

  return {
    refresh(): void {
      void load();
    },
    create(input: ExerciseInput): void {
      void create(input);
    },
    cancel(): void {
      loadToken += 1;
      mutateToken += 1;
      if (busy) {
        busy = false;
        deps.emit({ type: "superseded", of: "mutation" });
      }
    },
  };
}

/** Coordinate exercise library state for the Fitness Tab UI. */
export function useFitness(api: FitnessApi | null): readonly [FitnessState, FitnessActions] {
  const [state, dispatch] = useReducer(reduce, { tag: "loading" });
  const controller = useRef<ReturnType<typeof createFitnessController> | null>(null);

  useEffect(() => {
    if (api === null) {
      controller.current = null;
      return;
    }
    const current = createFitnessController({ api, emit: dispatch });
    controller.current = current;
    current.refresh();
    return () => current.cancel();
  }, [api]);

  return [
    state,
    useMemo<FitnessActions>(
      () => ({
        refresh: () => controller.current?.refresh(),
        create: (input) => controller.current?.create(input),
      }),
      [],
    ),
  ];
}

/** Apply a machine event to exercise library state. */
export function reduce(state: FitnessState, event: FitnessEvent): FitnessState {
  switch (event.type) {
    case "loadStarted":
      return state.tag === "ready" ? { ...state, refreshing: true } : { tag: "loading" };
    case "loadSucceeded":
      return {
        tag: "ready",
        exercises: event.exercises,
        refreshing: false,
        busy: state.tag === "ready" ? state.busy : false,
        error: null,
      };
    case "loadFailed":
      return state.tag === "ready"
        ? { ...state, refreshing: false, error: event.message }
        : { tag: "error", message: event.message };
    case "createStarted":
      return state.tag === "ready" ? { ...state, busy: true, error: null } : state;
    case "createSucceeded":
      return state.tag === "ready" ? { ...state, busy: false, error: null } : state;
    case "createFailed":
      return state.tag === "ready" ? { ...state, busy: false, error: event.message } : state;
    case "superseded":
      return state.tag === "ready" ? { ...state, busy: false } : state;
  }
}
