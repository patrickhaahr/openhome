import { describe, expect, it } from "vitest";

import type { AdguardStatus } from "../domain/adguard";
import { failure, success, type Result } from "../domain/result";
import {
  createAdguardController,
  reduce,
  type AdguardEvent,
  type AdguardState,
} from "./use-adguard";
import type { AdguardApi } from "../infrastructure/open-home-api";

const protectedStatus: AdguardStatus = {
  version: "v0.107.52",
  running: true,
  protectionEnabled: true,
  pauseEndsAtMs: null,
};

const pausedStatus: AdguardStatus = {
  version: "v0.107.52",
  running: true,
  protectionEnabled: false,
  pauseEndsAtMs: Date.now() + 15 * 60_000,
};

/** A scripted fake of the Axum API adapter interface with manually resolved responses. */
function fakeApi() {
  const calls: string[] = [];
  const pending: Array<{
    method: string;
    resolve: (result: Result<AdguardStatus>) => void;
  }> = [];
  const api: AdguardApi = {
    getStatus: () => record("status"),
    enableProtection: () => record("enable"),
    disableProtection: () => record("disable"),
    pauseProtection: (minutes) => record(`pause:${minutes}`),
  };

  function record(method: string): Promise<Result<AdguardStatus>> {
    calls.push(method);
    return new Promise((resolve) => pending.push({ method, resolve }));
  }

  return { api, calls, pending };
}

function harness() {
  const { api, calls, pending } = fakeApi();
  const events: AdguardEvent[] = [];
  const controller = createAdguardController({
    api,
    emit: (event) => events.push(event),
  });
  return { controller, calls, pending, events };
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function replay(events: ReadonlyArray<AdguardEvent>, initial: AdguardState): AdguardState {
  return events.reduce((state, event) => reduce(state, event), initial);
}

const initial: AdguardState = { tag: "loading" };

describe("adguard state machine", () => {
  it("loads status into a ready card", async () => {
    const h = harness();
    h.controller.refresh();
    expect(h.calls).toEqual(["status"]);

    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    expect(h.events.map((event) => event.type)).toEqual(["loadStarted", "loadSucceeded"]);
    const state = replay(h.events, initial);
    expect(state).toEqual({
      tag: "ready",
      status: protectedStatus,
      refreshing: false,
      acting: false,
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

  it("refreshes without dropping the visible status", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    h.controller.refresh();
    const refreshingState = replay(h.events, initial);
    expect(refreshingState.tag === "ready" && refreshingState.status).toEqual(protectedStatus);
    expect(refreshingState.tag === "ready" && refreshingState.refreshing).toBe(true);

    h.pending[1]?.resolve(success(pausedStatus));
    await settle();
    const refreshed = replay(h.events, initial);
    expect(refreshed.tag === "ready" && refreshed.status).toEqual(pausedStatus);
  });

  it("keeps the card and reports an inline error when a refresh fails", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    h.controller.refresh();
    h.pending[1]?.resolve(failure("Couldn't reach the Axum API."));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.status).toEqual(protectedStatus);
    expect(state.tag === "ready" && state.refreshing).toBe(false);
    expect(state.tag === "ready" && state.error).toBe("Couldn't reach the Axum API.");
  });

  it("pauses protection for the requested minutes and updates the card", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    h.controller.pause(15);
    expect(h.calls).toEqual(["status", "pause:15"]);

    h.pending[1]?.resolve(success(pausedStatus));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.status).toEqual(pausedStatus);
    expect(state.tag === "ready" && state.acting).toBe(false);
  });

  it("reports an action failure inline without losing status", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    h.controller.disable();
    h.pending[1]?.resolve(failure("AdGuard rejected the request."));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.status).toEqual(protectedStatus);
    expect(state.tag === "ready" && state.acting).toBe(false);
    expect(state.tag === "ready" && state.error).toBe("AdGuard rejected the request.");
  });

  it("drops a superseded status response", async () => {
    const h = harness();
    h.controller.refresh();
    h.controller.refresh();

    h.pending[1]?.resolve(success(pausedStatus));
    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    const succeeded = h.events.filter((event) => event.type === "loadSucceeded");
    expect(succeeded).toHaveLength(1);
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.status).toEqual(pausedStatus);
  });

  it("ignores actions pressed while another action is in flight", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    h.controller.enable();
    h.controller.enable();
    expect(h.calls).toEqual(["status", "enable"]);
  });

  it("a superseded refresh does not clear an in-flight action's flag", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    h.controller.refresh();
    h.controller.disable();

    h.pending[1]?.resolve(success(pausedStatus));
    await settle();
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.refreshing).toBe(false);
    expect(state.tag === "ready" && state.acting).toBe(true);
  });

  it("cancelling drops in-flight responses", async () => {
    const h = harness();
    h.controller.refresh();
    h.controller.cancel();

    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    expect(h.events.map((event) => event.type)).toEqual(["loadStarted", "superseded"]);
  });

  it("re-enables actions when a refresh supersedes an in-flight action", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    h.controller.disable();
    h.controller.refresh();
    expect(h.calls).toEqual(["status", "disable", "status"]);

    h.pending[1]?.resolve(success(pausedStatus));
    await settle();
    let state = replay(h.events, initial);
    expect(state.tag === "ready" && state.acting).toBe(false);

    h.pending[2]?.resolve(success(pausedStatus));
    await settle();
    state = replay(h.events, initial);
    expect(state.tag === "ready" && state.status).toEqual(pausedStatus);
  });

  it("stops the refresh spinner when an action supersedes an in-flight refresh", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success(protectedStatus));
    await settle();

    h.controller.refresh();
    h.controller.enable();

    h.pending[1]?.resolve(success(protectedStatus));
    await settle();
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.refreshing).toBe(false);

    h.pending[2]?.resolve(success(pausedStatus));
    await settle();
    const settledState = replay(h.events, initial);
    expect(settledState.tag === "ready" && settledState.acting).toBe(false);
    expect(settledState.tag === "ready" && settledState.status).toEqual(pausedStatus);
  });
});
