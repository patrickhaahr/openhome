import { describe, expect, it } from "vitest";

import type { DockerContainer } from "../domain/docker";
import { failure, success, type Result } from "../domain/result";
import {
  createDockerController,
  reduce,
  type DockerEvent,
  type DockerState,
} from "./use-docker";
import type { DockerApi } from "../infrastructure/open-home-api";

const containers: readonly DockerContainer[] = [
  {
    name: "adguard",
    state: "running",
    health: "healthy",
    image: "adguard/adguardhome",
    uptimeSeconds: 259_200,
    ports: ["53:53/tcp"],
    restartCount: 2,
  },
];

/** A scripted fake of the Axum API adapter interface with manually resolved responses. */
function fakeApi() {
  const calls: string[] = [];
  const pending: Array<{ resolve: (result: Result<readonly DockerContainer[]>) => void }> = [];
  const actionPending: Array<{ resolve: (result: Result<void>) => void }> = [];
  const logPending: Array<{ resolve: (result: Result<readonly string[]>) => void }> = [];
  const api: DockerApi = {
    listContainers: () => {
      calls.push("list");
      return new Promise((resolve) => pending.push({ resolve }));
    },
    startContainer: (name) => {
      calls.push(`start ${name}`);
      return new Promise((resolve) => actionPending.push({ resolve }));
    },
    stopContainer: (name) => {
      calls.push(`stop ${name}`);
      return new Promise((resolve) => actionPending.push({ resolve }));
    },
    restartContainer: (name) => {
      calls.push(`restart ${name}`);
      return new Promise((resolve) => actionPending.push({ resolve }));
    },
    containerLogs: (name) => {
      calls.push(`logs ${name}`);
      return new Promise((resolve) => logPending.push({ resolve }));
    },
  };
  return { api, calls, pending, actionPending, logPending };
}

function harness() {
  const { api, calls, pending, actionPending, logPending } = fakeApi();
  const events: DockerEvent[] = [];
  const controller = createDockerController({
    api,
    emit: (event) => events.push(event),
  });
  return { controller, calls, pending, actionPending, logPending, events };
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function replay(events: ReadonlyArray<DockerEvent>, initial: DockerState): DockerState {
  return events.reduce((state, event) => reduce(state, event), initial);
}

const initial: DockerState = { tag: "loading" };

describe("docker state machine", () => {
  it("loads containers into a ready list", async () => {
    const h = harness();
    h.controller.refresh();
    expect(h.calls).toEqual(["list"]);

    h.pending[0]?.resolve(success(containers));
    await settle();

    expect(h.events.map((event) => event.type)).toEqual(["loadStarted", "loadSucceeded"]);
    expect(replay(h.events, initial)).toEqual({
      tag: "ready",
      containers,
      refreshing: false,
      acting: null,
      error: null,
      view: { tag: "list" },
    });
  });

  it("surfaces a failed load as a retryable error", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(failure("Couldn't reach the Axum API."));
    await settle();

    expect(replay(h.events, initial)).toEqual({
      tag: "error",
      message: "Couldn't reach the Axum API.",
    });
  });

  it("refreshes without dropping the visible list", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(containers));
    await settle();

    h.controller.refresh();
    const refreshingState = replay(h.events, initial);
    expect(refreshingState.tag === "ready" && refreshingState.containers).toEqual(containers);
    expect(refreshingState.tag === "ready" && refreshingState.refreshing).toBe(true);

    const updated = [container()];
    h.pending[1]?.resolve(success(updated));
    await settle();
    const refreshed = replay(h.events, initial);
    expect(refreshed.tag === "ready" && refreshed.containers).toEqual(updated);
  });

  it("keeps the list and reports an inline error when a refresh fails", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(containers));
    await settle();

    h.controller.refresh();
    h.pending[1]?.resolve(failure("Couldn't reach the Axum API."));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.containers).toEqual(containers);
    expect(state.tag === "ready" && state.refreshing).toBe(false);
    expect(state.tag === "ready" && state.error).toBe("Couldn't reach the Axum API.");
  });

  it("dispatches a lifecycle action immediately without confirmation", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(containers));
    await settle();

    h.controller.stopContainer("adguard");
    expect(h.calls).toEqual(["list", "stop adguard"]);
    expect(replay(h.events, initial)).toMatchObject({
      tag: "ready",
      acting: { name: "adguard", action: "stop" },
    });
  });

  it("refreshes the list after a successful action", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(containers));
    await settle();

    h.controller.restartContainer("adguard");
    h.actionPending[0]?.resolve(success(undefined));
    await settle();

    expect(h.calls).toEqual(["list", "restart adguard", "list"]);

    const restarted: readonly DockerContainer[] = [
      {
        name: "adguard",
        state: "exited",
        health: null,
        image: "adguard/adguardhome",
        uptimeSeconds: null,
        ports: ["53:53/tcp"],
        restartCount: 2,
      },
    ];
    h.pending[1]?.resolve(success(restarted));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.containers).toEqual(restarted);
    expect(state.tag === "ready" && state.acting).toBeNull();
    expect(state.tag === "ready" && state.error).toBeNull();
  });

  it.each([
    "Container adguard not found.",
    "Docker unavailable.",
    "Docker daemon rejected the request.",
  ])("surfaces an action failure inline without dropping the list: %s", async (message) => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(containers));
    await settle();

    h.controller.startContainer("adguard");
    h.actionPending[0]?.resolve(failure(message));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.containers).toEqual(containers);
    expect(state.tag === "ready" && state.acting).toBeNull();
    expect(state.tag === "ready" && state.error).toBe(message);
    expect(h.calls).toEqual(["list", "start adguard"]);
  });

  it("drops a second action while one is in flight", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(containers));
    await settle();

    h.controller.startContainer("adguard");
    h.controller.stopContainer("adguard");
    h.actionPending[0]?.resolve(success(undefined));
    await settle();

    expect(h.calls).toEqual(["list", "start adguard", "list"]);
  });

  it("drops a cancelled action without refreshing", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(containers));
    await settle();

    h.controller.startContainer("adguard");
    h.controller.cancel();
    h.actionPending[0]?.resolve(success(undefined));
    await settle();

    expect(h.calls).toEqual(["list", "start adguard"]);
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.acting).toBeNull();
  });

  it("drops a superseded list response", async () => {
    const h = harness();
    h.controller.refresh();
    h.controller.refresh();

    const updated = [container()];
    h.pending[1]?.resolve(success(updated));
    h.pending[0]?.resolve(success(containers));
    await settle();

    const succeeded = h.events.filter((event) => event.type === "loadSucceeded");
    expect(succeeded).toHaveLength(1);
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.containers).toEqual(updated);
  });

  it("cancelling drops in-flight responses", async () => {
    const h = harness();
    h.controller.refresh();
    h.controller.cancel();

    h.pending[0]?.resolve(success(containers));
    await settle();

    expect(h.events.map((event) => event.type)).toEqual(["loadStarted", "superseded"]);
  });
});

describe("docker container logs machine", () => {
  const lines: readonly string[] = ["2026-08-29T12:00:00Z started", "2026-08-29T12:00:01Z ready"];

  /** Logs can only be opened from a loaded list, so replay from a ready base. */
  const readyInitial = replay(
    [
      { type: "loadStarted" },
      { type: "loadSucceeded", containers },
    ],
    initial,
  );

  function replayLogs(state: DockerState): Extract<DockerState, { tag: "ready" }>["view"] {
    return state.tag === "ready" ? state.view : { tag: "list" };
  }

  function loadedHarness(): ReturnType<typeof harness> {
    const h = harness();
    h.controller.openLogs("adguard");
    h.logPending[0]?.resolve(success(lines));
    return h;
  }

  it("loads a container's logs after selecting it", async () => {
    const h = harness();
    h.controller.openLogs("adguard");

    expect(h.calls).toEqual(["logs adguard"]);
    expect(replayLogs(replay(h.events, readyInitial))).toEqual({
      tag: "logs",
      name: "adguard",
      logs: { tag: "loading" },
    });

    h.logPending[0]?.resolve(success(lines));
    await settle();

    expect(h.events.map((event) => event.type)).toEqual([
      "logsOpened",
      "logsStarted",
      "logsLoaded",
    ]);
    expect(replayLogs(replay(h.events, readyInitial))).toEqual({
      tag: "logs",
      name: "adguard",
      logs: { tag: "ready", lines, refreshing: false, error: null },
    });
  });

  it("refreshes logs in place while keeping the visible lines", async () => {
    const h = loadedHarness();
    await settle();

    h.controller.refresh();
    expect(h.calls).toEqual(["logs adguard", "logs adguard"]);
    const refreshing = replayLogs(replay(h.events, readyInitial));
    expect(
      refreshing.tag === "logs" &&
        refreshing.logs.tag === "ready" && {
          lines: refreshing.logs.lines,
          refreshing: refreshing.logs.refreshing,
        },
    ).toEqual({ lines, refreshing: true });

    const updated = ["2026-08-29T12:00:02Z restarted"];
    h.logPending[1]?.resolve(success(updated));
    await settle();

    const refreshed = replayLogs(replay(h.events, readyInitial));
    expect(
      refreshed.tag === "logs" &&
        refreshed.logs.tag === "ready" && {
          lines: refreshed.logs.lines,
          refreshing: refreshed.logs.refreshing,
        },
    ).toEqual({ lines: updated, refreshing: false });
  });

  it("keeps the lines and reports an inline error when a logs refresh fails", async () => {
    const h = loadedHarness();
    await settle();

    h.controller.refresh();
    h.logPending[1]?.resolve(failure("Docker unavailable."));
    await settle();

    const state = replay(h.events, readyInitial);
    expect(
      state.tag === "ready" &&
        state.view.tag === "logs" &&
        state.view.logs.tag === "ready" && {
          lines: state.view.logs.lines,
          refreshing: state.view.logs.refreshing,
          error: state.view.logs.error,
        },
    ).toEqual({ lines, refreshing: false, error: "Docker unavailable." });
  });

  it.each([
    "Container adguard not found.",
    "Couldn't load the container logs.",
  ])("surfaces a failed first load as a retryable inline error: %s", async (message) => {
    const h = harness();
    h.controller.openLogs("adguard");
    h.logPending[0]?.resolve(failure(message));
    await settle();

    expect(replayLogs(replay(h.events, readyInitial))).toEqual({
      tag: "logs",
      name: "adguard",
      logs: { tag: "error", message },
    });

    h.controller.refresh();
    expect(replayLogs(replay(h.events, readyInitial))).toEqual({
      tag: "logs",
      name: "adguard",
      logs: { tag: "loading" },
    });
  });

  it("back returns to the unfiltered list and drops a stale in-flight response", async () => {
    const h = harness();
    h.controller.openLogs("adguard");
    await settle();
    h.logPending[0]?.resolve(success(lines));
    await settle();

    h.controller.refresh();
    h.controller.closeLogs();
    h.logPending[1]?.resolve(success(["late"]));
    await settle();

    expect(h.calls).toEqual(["logs adguard", "logs adguard"]);
    expect(h.events).toContainEqual({ type: "logsClosed" });
    expect(replayLogs(replay(h.events, readyInitial))).toEqual({ tag: "list" });

    h.controller.openLogs("feed-rs");
    expect(h.calls).toEqual(["logs adguard", "logs adguard", "logs feed-rs"]);
    expect(replayLogs(replay(h.events, readyInitial))).toEqual({
      tag: "logs",
      name: "feed-rs",
      logs: { tag: "loading" },
    });
  });

  it("drops a superseded logs response and keeps the latest selection", async () => {
    const h = harness();
    h.controller.openLogs("adguard");
    h.controller.openLogs("feed-rs");

    h.logPending[0]?.resolve(success(["stale"]));
    await settle();

    const latest = ["2026-08-29T12:00:00Z feed"];
    h.logPending[1]?.resolve(success(latest));
    await settle();

    expect(replayLogs(replay(h.events, readyInitial))).toEqual({
      tag: "logs",
      name: "feed-rs",
      logs: { tag: "ready", lines: latest, refreshing: false, error: null },
    });
  });

  it("routes refresh to the list while no logs are open", async () => {
    const h = harness();
    h.controller.openLogs("adguard");
    await settle();
    h.logPending[0]?.resolve(success(lines));
    await settle();

    h.controller.closeLogs();
    h.controller.refresh();

    expect(h.calls).toEqual(["logs adguard", "list"]);
  });
});

function container(): DockerContainer {
  return {
    name: "feed-rs",
    state: "exited",
    health: null,
    image: "feed-rs:latest",
    uptimeSeconds: null,
    ports: [],
    restartCount: 0,
  };
}
