import { useEffect, useMemo, useReducer, useRef } from "react";

import type { WorkoutDetail, WorkoutInput, WorkoutSummary } from "../domain/fitness";
import type { FitnessApi } from "../infrastructure/open-home-api";
import type { Result } from "../domain/result";

/** The workout log/history view with its detail drill-down. */
export type WorkoutsState = {
  readonly workouts: readonly WorkoutSummary[];
  /** A history refresh is in flight; the previous list stays visible. */
  readonly refreshing: boolean;
  /** A save or delete is in flight. */
  readonly saving: boolean;
  /** The opened workout detail, if any. */
  readonly detail:
    | { readonly tag: "closed" }
    | { readonly tag: "loading" }
    | { readonly tag: "loaded"; readonly workout: WorkoutDetail }
    | { readonly tag: "error"; readonly message: string };
  readonly error: string | null;
};

/** A workout save request; `update` targets an existing workout by id. */
export type WorkoutSaveRequest =
  | { readonly tag: "create"; readonly input: WorkoutInput }
  | { readonly tag: "update"; readonly id: number; readonly input: WorkoutInput };

/** User actions accepted by the workout log/history view. */
export type WorkoutsActions = {
  readonly refresh: () => void;
  readonly openWorkout: (id: number) => void;
  readonly closeWorkout: () => void;
  /** The input is validated on the client by the caller before this is invoked. */
  readonly saveWorkout: (request: WorkoutSaveRequest) => void;
  readonly deleteWorkout: (id: number) => void;
};

export type WorkoutsEvent =
  | { readonly type: "started" }
  | { readonly type: "succeeded"; readonly workouts: readonly WorkoutSummary[] }
  | { readonly type: "failed"; readonly message: string }
  | { readonly type: "detailStarted" }
  | { readonly type: "detailSucceeded"; readonly workout: WorkoutDetail }
  | { readonly type: "detailFailed"; readonly message: string }
  | { readonly type: "detailClosed" }
  | { readonly type: "saveStarted" }
  | { readonly type: "saveSucceeded" }
  | { readonly type: "saveFailed"; readonly message: string }
  /** A cancelled controller's in-flight mutation resolved; its flag must clear. */
  | { readonly type: "superseded" };

/** Drives the workout log/history view against the Axum API adapter, emitting events. */
export function createWorkoutsController(deps: {
  readonly api: Pick<
    FitnessApi,
    "listWorkouts" | "getWorkout" | "createWorkout" | "updateWorkout" | "deleteWorkout"
  >;
  readonly emit: (event: WorkoutsEvent) => void;
}) {
  let listToken = 0;
  let detailToken = 0;
  let mutateToken = 0;
  let busy = false;

  async function load(): Promise<void> {
    const current = ++listToken;
    deps.emit({ type: "started" });
    const result = await deps.api.listWorkouts();
    if (listToken !== current) {
      return;
    }
    deps.emit(
      result.ok
        ? { type: "succeeded", workouts: result.value }
        : { type: "failed", message: result.error },
    );
  }

  async function openWorkout(id: number): Promise<void> {
    const current = ++detailToken;
    deps.emit({ type: "detailStarted" });
    const result = await deps.api.getWorkout(id);
    if (detailToken !== current) {
      return;
    }
    deps.emit(
      result.ok
        ? { type: "detailSucceeded", workout: result.value }
        : { type: "detailFailed", message: result.error },
    );
  }

  /** Run one workout mutation, reloading history afterwards when it succeeded. */
  async function mutate(run: () => Promise<Result<unknown>>): Promise<void> {
    if (busy) {
      return;
    }
    busy = true;
    const current = ++mutateToken;
    // A list refresh in flight since before this mutation carries a stale
    // snapshot; drop it so the post-mutation reload is the authoritative one.
    listToken += 1;
    deps.emit({ type: "saveStarted" });
    const result = await run();
    if (mutateToken !== current) {
      busy = false;
      deps.emit({ type: "superseded" });
      return;
    }
    busy = false;
    if (!result.ok) {
      deps.emit({ type: "saveFailed", message: result.error });
      return;
    }
    deps.emit({ type: "saveSucceeded" });
    void load();
  }

  return {
    refresh(): void {
      void load();
    },
    openWorkout(id: number): void {
      void openWorkout(id);
    },
    closeWorkout(): void {
      deps.emit({ type: "detailClosed" });
    },
    saveWorkout(request: WorkoutSaveRequest): void {
      void mutate(() =>
        request.tag === "create"
          ? deps.api.createWorkout(request.input)
          : deps.api.updateWorkout(request.id, request.input),
      );
    },
    deleteWorkout(id: number): void {
      void mutate(() => deps.api.deleteWorkout(id));
    },
    cancel(): void {
      listToken += 1;
      detailToken += 1;
      mutateToken += 1;
      if (busy) {
        busy = false;
        deps.emit({ type: "superseded" });
      }
    },
  };
}

/** Coordinate workout log/history state for the Fitness Tab UI. */
export function useWorkouts(api: FitnessApi | null): readonly [WorkoutsState, WorkoutsActions] {
  const [state, dispatch] = useReducer(reduce, initialState);
  const controller = useRef<ReturnType<typeof createWorkoutsController> | null>(null);

  useEffect(() => {
    if (api === null) {
      controller.current = null;
      return;
    }
    const current = createWorkoutsController({ api, emit: dispatch });
    controller.current = current;
    current.refresh();
    return () => current.cancel();
  }, [api]);

  return [
    state,
    useMemo<WorkoutsActions>(
      () => ({
        refresh: () => controller.current?.refresh(),
        openWorkout: (id) => controller.current?.openWorkout(id),
        closeWorkout: () => controller.current?.closeWorkout(),
        saveWorkout: (request) => controller.current?.saveWorkout(request),
        deleteWorkout: (id) => controller.current?.deleteWorkout(id),
      }),
      [],
    ),
  ];
}

export const initialState: WorkoutsState = {
  workouts: [],
  refreshing: false,
  saving: false,
  detail: { tag: "closed" },
  error: null,
};

/** Apply a machine event to workout log/history state. */
export function reduce(state: WorkoutsState, event: WorkoutsEvent): WorkoutsState {
  switch (event.type) {
    case "started":
      return { ...state, refreshing: true };
    case "succeeded":
      return { ...state, workouts: event.workouts, refreshing: false, error: null };
    case "failed":
      return { ...state, refreshing: false, error: event.message };
    case "detailStarted":
      return { ...state, detail: { tag: "loading" } };
    case "detailSucceeded":
      return { ...state, detail: { tag: "loaded", workout: event.workout } };
    case "detailFailed":
      return { ...state, detail: { tag: "error", message: event.message } };
    case "detailClosed":
      return { ...state, detail: { tag: "closed" } };
    case "saveStarted":
      return { ...state, saving: true, error: null };
    case "saveSucceeded":
      // The history reload that follows carries the fresh list; close the
      // detail so the UI returns to history after saving from either form.
      return { ...state, saving: false, detail: { tag: "closed" }, error: null };
    case "saveFailed":
      return { ...state, saving: false, error: event.message };
    case "superseded":
      return { ...state, saving: false };
  }
}
