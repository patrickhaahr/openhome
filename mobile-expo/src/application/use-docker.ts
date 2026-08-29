import { useEffect, useMemo, useReducer, useRef } from "react";

import {
  classificationCounts,
  type ClassificationCounts,
  type DockerContainer,
} from "../domain/docker";
import type { DockerApi } from "../infrastructure/open-home-api";

/** The user-visible state of the Docker Tab. */
export type DockerState =
  | { readonly tag: "loading" }
  | { readonly tag: "error"; readonly message: string }
  | {
      readonly tag: "ready";
      readonly containers: readonly DockerContainer[];
      /** A list refresh is in flight; the previous list stays visible. */
      readonly refreshing: boolean;
      readonly error: string | null;
    };

/** User actions accepted by the Docker Tab. */
export type DockerActions = {
  readonly refresh: () => void;
};

export type DockerEvent =
  | { readonly type: "loadStarted" }
  | { readonly type: "loadSucceeded"; readonly containers: readonly DockerContainer[] }
  | { readonly type: "loadFailed"; readonly message: string }
  /** A superseded load resolved; its result is dropped. */
  | { readonly type: "superseded" };

const emptyCounts: ClassificationCounts = { all: 0, healthy: 0, unhealthy: 0, stopped: 0 };

/** Drives the Docker machine against the Axum API adapter, emitting events. */
export function createDockerController(deps: {
  readonly api: DockerApi;
  readonly emit: (event: DockerEvent) => void;
}) {
  let token = 0;

  async function fetchContainers(): Promise<void> {
    const current = ++token;
    deps.emit({ type: "loadStarted" });
    const result = await deps.api.listContainers();
    if (token !== current) {
      deps.emit({ type: "superseded" });
      return;
    }
    deps.emit(
      result.ok
        ? { type: "loadSucceeded", containers: result.value }
        : { type: "loadFailed", message: result.error },
    );
  }

  return {
    refresh(): void {
      void fetchContainers();
    },
    cancel(): void {
      token += 1;
    },
  };
}

/**
 * Coordinate Docker container state for the Docker Tab and the Server Tab's
 * Docker Health Summary. Exposes per-classification counts alongside the state.
 */
export function useDocker(
  api: DockerApi | null,
): readonly [DockerState, DockerActions, ClassificationCounts] {
  const [state, dispatch] = useReducer(reduce, { tag: "loading" });
  const controller = useRef<ReturnType<typeof createDockerController> | null>(null);

  useEffect(() => {
    if (api === null) {
      controller.current = null;
      return;
    }
    const current = createDockerController({ api, emit: dispatch });
    controller.current = current;
    current.refresh();
    return () => current.cancel();
  }, [api]);

  const counts = useMemo(
    () => (state.tag === "ready" ? classificationCounts(state.containers) : emptyCounts),
    [state],
  );

  return [
    state,
    useMemo<DockerActions>(() => ({ refresh: () => controller.current?.refresh() }), []),
    counts,
  ];
}

/** Apply a machine event to Docker Tab state. */
export function reduce(state: DockerState, event: DockerEvent): DockerState {
  switch (event.type) {
    case "loadStarted":
      return state.tag === "ready" ? { ...state, refreshing: true } : { tag: "loading" };
    case "loadSucceeded":
      return {
        tag: "ready",
        containers: event.containers,
        refreshing: false,
        error: null,
      };
    case "loadFailed":
      return state.tag === "ready"
        ? { ...state, refreshing: false, error: event.message }
        : { tag: "error", message: event.message };
    case "superseded":
      return state.tag === "ready" ? { ...state, refreshing: false } : state;
  }
}
