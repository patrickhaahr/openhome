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
  const api: DockerApi = {
    listContainers: () => {
      calls.push("list");
      return new Promise((resolve) => pending.push({ resolve }));
    },
  };
  return { api, calls, pending };
}

function harness() {
  const { api, calls, pending } = fakeApi();
  const events: DockerEvent[] = [];
  const controller = createDockerController({
    api,
    emit: (event) => events.push(event),
  });
  return { controller, calls, pending, events };
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
      error: null,
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
