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
      /** The container lifecycle action in flight, if any. */
      readonly acting: { readonly name: string; readonly action: ContainerAction } | null;
      readonly error: string | null;
    };

/** One immediate container lifecycle action. */
export type ContainerAction = "start" | "stop" | "restart";

/** User actions accepted by the Docker Tab. */
export type DockerActions = {
  readonly refresh: () => void;
  readonly startContainer: (name: string) => void;
  readonly stopContainer: (name: string) => void;
  readonly restartContainer: (name: string) => void;
};

export type DockerEvent =
  | { readonly type: "loadStarted" }
  | { readonly type: "loadSucceeded"; readonly containers: readonly DockerContainer[] }
  | { readonly type: "loadFailed"; readonly message: string }
  | { readonly type: "actionStarted"; readonly name: string; readonly action: ContainerAction }
  | { readonly type: "actionFinished"; readonly error: string | null }
  /** A superseded operation resolved; its result is dropped but its own flag must clear. */
  | { readonly type: "superseded"; readonly of: "refresh" | "action" };

const emptyCounts: ClassificationCounts = { all: 0, healthy: 0, unhealthy: 0, stopped: 0 };

/** Drives the Docker machine against the Axum API adapter, emitting events. */
export function createDockerController(deps: {
  readonly api: DockerApi;
  readonly emit: (event: DockerEvent) => void;
}) {
  let token = 0;
  let acting = false;

  async function fetchContainers(): Promise<void> {
    const current = ++token;
    deps.emit({ type: "loadStarted" });
    const result = await deps.api.listContainers();
    if (token !== current) {
      deps.emit({ type: "superseded", of: "refresh" });
      return;
    }
    deps.emit(
      result.ok
        ? { type: "loadSucceeded", containers: result.value }
        : { type: "loadFailed", message: result.error },
    );
  }

  async function act(name: string, action: ContainerAction): Promise<void> {
    if (acting) {
      return;
    }
    acting = true;
    const current = ++token;
    deps.emit({ type: "actionStarted", name, action });
    const result = await deps.api[`${action}Container`](name);
    acting = false;
    if (token !== current) {
      deps.emit({ type: "superseded", of: "action" });
      return;
    }
    deps.emit({ type: "actionFinished", error: result.ok ? null : result.error });
    if (result.ok) {
      void fetchContainers();
    }
  }

  return {
    refresh(): void {
      void fetchContainers();
    },
    startContainer(name: string): void {
      void act(name, "start");
    },
    stopContainer(name: string): void {
      void act(name, "stop");
    },
    restartContainer(name: string): void {
      void act(name, "restart");
    },
    cancel(): void {
      token += 1;
      acting = false;
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
    useMemo<DockerActions>(
      () => ({
        refresh: () => controller.current?.refresh(),
        startContainer: (name) => controller.current?.startContainer(name),
        stopContainer: (name) => controller.current?.stopContainer(name),
        restartContainer: (name) => controller.current?.restartContainer(name),
      }),
      [],
    ),
    counts,
  ];
}

/** Apply a machine event to Docker Tab state. */
export function reduce(state: DockerState, event: DockerEvent): DockerState {
  switch (event.type) {
    case "loadStarted":
      return state.tag === "ready" ? { ...state, refreshing: true } : { tag: "loading" };
    case "loadSucceeded":
      return state.tag === "ready"
        ? { ...state, containers: event.containers, refreshing: false, error: null }
        : {
            tag: "ready",
            containers: event.containers,
            refreshing: false,
            acting: null,
            error: null,
          };
    case "loadFailed":
      return state.tag === "ready"
        ? { ...state, refreshing: false, error: event.message }
        : { tag: "error", message: event.message };
    case "actionStarted":
      return state.tag === "ready"
        ? { ...state, acting: { name: event.name, action: event.action }, error: null }
        : state;
    case "actionFinished":
      return state.tag === "ready" ? { ...state, acting: null, error: event.error } : state;
    case "superseded":
      return state.tag === "ready"
        ? event.of === "refresh"
          ? { ...state, refreshing: false }
          : { ...state, acting: null }
        : state;
  }
}
