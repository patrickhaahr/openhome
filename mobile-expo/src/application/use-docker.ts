import { useEffect, useMemo, useReducer, useRef } from "react";

import {
  classificationCounts,
  type ClassificationCounts,
  type DockerContainer,
} from "../domain/docker";
import type { DockerApi } from "../infrastructure/open-home-api";

/** The state of the container logs view inside the Docker Tab. */
export type ContainerLogsState =
  | { readonly tag: "loading" }
  | { readonly tag: "error"; readonly message: string }
  | {
      readonly tag: "ready";
      readonly lines: readonly string[];
      /** A logs refresh is in flight; the previous lines stay visible. */
      readonly refreshing: boolean;
      readonly error: string | null;
    };

/** Which surface the Docker Tab renders: the container list or one container's logs. */
export type DockerView =
  | { readonly tag: "list" }
  | { readonly tag: "logs"; readonly name: string; readonly logs: ContainerLogsState };

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
      readonly view: DockerView;
    };

/** One immediate container lifecycle action. */
export type ContainerAction = "start" | "stop" | "restart";

/** User actions accepted by the Docker Tab. */
export type DockerActions = {
  readonly refresh: () => void;
  readonly openLogs: (name: string) => void;
  readonly closeLogs: () => void;
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
  | { readonly type: "logsOpened"; readonly name: string }
  | { readonly type: "logsStarted" }
  | { readonly type: "logsLoaded"; readonly lines: readonly string[] }
  | { readonly type: "logsFailed"; readonly message: string }
  | { readonly type: "logsClosed" }
  /** A superseded operation resolved; its result is dropped but its own flag must clear. */
  | { readonly type: "superseded"; readonly of: "refresh" | "action" | "logs" };

const emptyCounts: ClassificationCounts = { all: 0, healthy: 0, unhealthy: 0, stopped: 0 };

/** Drives the Docker machine against the Axum API adapter, emitting events. */
export function createDockerController(deps: {
  readonly api: DockerApi;
  readonly emit: (event: DockerEvent) => void;
}) {
  let token = 0;
  let acting = false;
  let logsToken = 0;
  let selected: string | null = null;

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

  async function fetchLogs(name: string): Promise<void> {
    const current = ++logsToken;
    deps.emit({ type: "logsStarted" });
    const result = await deps.api.containerLogs(name);
    if (logsToken !== current) {
      deps.emit({ type: "superseded", of: "logs" });
      return;
    }
    deps.emit(
      result.ok
        ? { type: "logsLoaded", lines: result.value }
        : { type: "logsFailed", message: result.error },
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
      if (selected !== null) {
        void fetchLogs(selected);
      } else {
        void fetchContainers();
      }
    },
    openLogs(name: string): void {
      selected = name;
      deps.emit({ type: "logsOpened", name });
      void fetchLogs(name);
    },
    closeLogs(): void {
      selected = null;
      logsToken += 1;
      deps.emit({ type: "logsClosed" });
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
      logsToken += 1;
      acting = false;
      selected = null;
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
    dispatch({ type: "logsClosed" });
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
        openLogs: (name) => controller.current?.openLogs(name),
        closeLogs: () => controller.current?.closeLogs(),
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
            view: { tag: "list" },
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
    case "logsOpened":
      return state.tag === "ready"
        ? { ...state, view: { tag: "logs", name: event.name, logs: { tag: "loading" } } }
        : state;
    case "logsStarted":
      return updateLogs(state, (logs) =>
        logs.tag === "ready" ? { ...logs, refreshing: true } : { tag: "loading" },
      );
    case "logsLoaded":
      return updateLogs(state, () => ({
        tag: "ready",
        lines: event.lines,
        refreshing: false,
        error: null,
      }));
    case "logsFailed":
      return updateLogs(state, (logs) =>
        logs.tag === "ready"
          ? { ...logs, refreshing: false, error: event.message }
          : { tag: "error", message: event.message },
      );
    case "logsClosed":
      return state.tag === "ready" ? { ...state, view: { tag: "list" } } : state;
    case "superseded":
      if (event.of === "logs") {
        return updateLogs(state, (logs) => (logs.tag === "ready" ? { ...logs, refreshing: false } : logs));
      }
      return state.tag === "ready"
        ? event.of === "refresh"
          ? { ...state, refreshing: false }
          : { ...state, acting: null }
        : state;
  }
}

/** Apply a logs-state update to the Docker Tab state when the logs view is open. */
function updateLogs(
  state: DockerState,
  update: (logs: ContainerLogsState) => ContainerLogsState,
): DockerState {
  return state.tag === "ready" && state.view.tag === "logs"
    ? { ...state, view: { ...state.view, logs: update(state.view.logs) } }
    : state;
}
