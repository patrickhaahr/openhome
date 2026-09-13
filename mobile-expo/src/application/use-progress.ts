import { useEffect, useMemo, useReducer, useRef } from "react";

import type { ExerciseProgress } from "../domain/fitness";
import type { FitnessApi } from "../infrastructure/open-home-api";

/** The per-exercise progress view with its selection. */
export type ProgressState =
  | { readonly tag: "idle"; readonly message: string }
  | { readonly tag: "loading"; readonly exerciseId: number }
  | { readonly tag: "loaded"; readonly progress: ExerciseProgress }
  | { readonly tag: "error"; readonly exerciseId: number; readonly message: string };

/** User actions accepted by the progress view. */
export type ProgressActions = {
  readonly openExercise: (id: number) => void;
  readonly refresh: () => void;
};

export type ProgressEvent =
  | { readonly type: "started"; readonly exerciseId: number }
  | { readonly type: "succeeded"; readonly progress: ExerciseProgress }
  | { readonly type: "failed"; readonly message: string };

/** Drives the progress view against the Axum API adapter, emitting events. */
export function createProgressController(deps: {
  readonly api: Pick<FitnessApi, "getExerciseProgress">;
  readonly emit: (event: ProgressEvent) => void;
}) {
  let token = 0;
  let currentId: number | null = null;

  async function load(id: number): Promise<void> {
    const current = ++token;
    currentId = id;
    deps.emit({ type: "started", exerciseId: id });
    const result = await deps.api.getExerciseProgress(id);
    if (token !== current) {
      return;
    }
    deps.emit(
      result.ok
        ? { type: "succeeded", progress: result.value }
        : { type: "failed", message: result.error },
    );
  }

  return {
    openExercise(id: number): void {
      void load(id);
    },
    refresh(): void {
      if (currentId !== null) {
        void load(currentId);
      }
    },
    cancel(): void {
      token += 1;
      currentId = null;
    },
  };
}

/** Coordinate per-exercise progress state for the Fitness Tab UI. */
export function useProgress(api: FitnessApi | null): readonly [ProgressState, ProgressActions] {
  const [state, dispatch] = useReducer(reduce, initialState);
  const controller = useRef<ReturnType<typeof createProgressController> | null>(null);

  useEffect(() => {
    if (api === null) {
      controller.current = null;
      return;
    }
    const current = createProgressController({ api, emit: dispatch });
    controller.current = current;
    return () => current.cancel();
  }, [api]);

  return [
    state,
    useMemo<ProgressActions>(
      () => ({
        openExercise: (id) => controller.current?.openExercise(id),
        refresh: () => controller.current?.refresh(),
      }),
      [],
    ),
  ];
}

export const initialState: ProgressState = { tag: "idle", message: "Pick an exercise to see its progress." };

/** Apply a machine event to progress view state. */
export function reduce(state: ProgressState, event: ProgressEvent): ProgressState {
  switch (event.type) {
    case "started":
      return { tag: "loading", exerciseId: event.exerciseId };
    case "succeeded":
      return { tag: "loaded", progress: event.progress };
    case "failed":
      return {
        tag: "error",
        exerciseId: state.tag === "loading" || state.tag === "error" ? state.exerciseId : 0,
        message: event.message,
      };
  }
}
