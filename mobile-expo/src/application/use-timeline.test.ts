import { describe, expect, it } from "vitest";

import type { TimelineItem } from "../domain/rss";
import { failure, success, type Result } from "../domain/result";
import {
  createTimelineController,
  reduce,
  TIMELINE_PAGE_SIZE,
  type TimelineEvent,
  type TimelineState,
} from "./use-timeline";

function item(id: number): TimelineItem {
  return { id, title: `Item ${id}`, description: null, link: `https://example.test/${id}` };
}

/** A scripted fake of the rss adapter interface with manually resolved responses. */
function fakeApi() {
  const requests: Array<{ beforeId: number | null; limit: number }> = [];
  const pending: Array<{ resolve: (result: Result<readonly TimelineItem[]>) => void }> = [];
  const api = {
    compactTimeline: (beforeId: number | null, limit: number) => {
      requests.push({ beforeId, limit });
      return new Promise<Result<readonly TimelineItem[]>>((resolve) => {
        pending.push({ resolve });
      });
    },
  };
  return { api, requests, pending };
}

function harness() {
  const { api, requests, pending } = fakeApi();
  const events: TimelineEvent[] = [];
  const controller = createTimelineController({
    api,
    emit: (event) => events.push(event),
  });
  return { controller, requests, pending, events };
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function replay(events: ReadonlyArray<TimelineEvent>, initial: TimelineState): TimelineState {
  return events.reduce((state, event) => reduce(state, event), initial);
}

const initial: TimelineState = { tag: "loading" };

/** Resolve the load started by refresh() with `count` fresh items. */
async function loadInitial(
  h: ReturnType<typeof harness>,
  count: number,
): Promise<void> {
  const ids = Array.from({ length: count }, (_, index) => index + 1);
  h.pending[0]?.resolve(success(ids.map(item)));
  await settle();
}

describe("timeline state machine", () => {
  it("loads the newest-first initial page into a ready list", async () => {
    const h = harness();
    h.controller.refresh();
    expect(h.requests).toEqual([{ beforeId: null, limit: TIMELINE_PAGE_SIZE }]);

    await loadInitial(h, 2);

    expect(h.events.map((event) => event.type)).toEqual(["loadStarted", "loadSucceeded"]);
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.items.map((entry) => entry.id)).toEqual([1, 2]);
    expect(state.tag === "ready" && state.hasMore).toBe(false);
  });

  it("renders an empty ready list when the timeline has no items", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(success([]));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.items).toEqual([]);
    expect(state.tag === "ready" && state.hasMore).toBe(false);
    expect(state.tag === "ready" && state.error).toBeNull();
  });

  it("surfaces a failed initial load as a retryable error", async () => {
    const h = harness();
    h.controller.refresh();
    h.pending[0]?.resolve(failure("Couldn't reach the Axum API."));
    await settle();

    expect(replay(h.events, initial)).toEqual({
      tag: "error",
      message: "Couldn't reach the Axum API.",
    });

    h.controller.refresh();
    h.pending[1]?.resolve(success([item(1)]));
    await settle();
    expect(replay(h.events, initial).tag).toBe("ready");
  });

  it("loads the next page using the last item's id when the list end is reached", async () => {
    const h = harness();
    h.controller.refresh();
    await loadInitial(h, TIMELINE_PAGE_SIZE);

    h.controller.loadMore();
    expect(h.requests[1]).toEqual({ beforeId: TIMELINE_PAGE_SIZE, limit: TIMELINE_PAGE_SIZE });

    h.pending[1]?.resolve(
      success(Array.from({ length: 3 }, (_, index) => item(TIMELINE_PAGE_SIZE + 1 + index))),
    );
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.items).toHaveLength(TIMELINE_PAGE_SIZE + 3);
    expect(
      state.tag === "ready" && state.items.map((entry) => entry.id).slice(-3),
    ).toEqual([TIMELINE_PAGE_SIZE + 1, TIMELINE_PAGE_SIZE + 2, TIMELINE_PAGE_SIZE + 3]);
  });

  it("does not re-request a page while one is in flight or after the list ends", async () => {
    const h = harness();
    h.controller.refresh();
    await loadInitial(h, TIMELINE_PAGE_SIZE);

    h.controller.loadMore();
    h.controller.loadMore();
    expect(h.requests).toHaveLength(2);

    h.pending[1]?.resolve(success([item(TIMELINE_PAGE_SIZE + 1)]));
    await settle();

    h.controller.loadMore();
    expect(h.requests).toHaveLength(2);
  });

  it("keeps newest-first ordering without duplicates across pages", async () => {
    const h = harness();
    h.controller.refresh();
    await loadInitial(h, TIMELINE_PAGE_SIZE);

    h.controller.loadMore();
    h.pending[1]?.resolve(success([item(TIMELINE_PAGE_SIZE), item(TIMELINE_PAGE_SIZE + 1)]));
    await settle();

    const state = replay(h.events, initial);
    const ids = state.tag === "ready" && state.items.map((entry) => entry.id);
    expect(ids).toEqual(
      Array.from({ length: TIMELINE_PAGE_SIZE + 1 }, (_, index) => index + 1),
    );
  });

  it("stops pagination once a page comes back short", async () => {
    const h = harness();
    h.controller.refresh();
    await loadInitial(h, TIMELINE_PAGE_SIZE);

    h.controller.loadMore();
    h.pending[1]?.resolve(success([item(TIMELINE_PAGE_SIZE + 1)]));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.hasMore).toBe(false);

    h.controller.loadMore();
    expect(h.requests).toHaveLength(2);
  });

  it("reports a load-more failure inline without dropping rendered items", async () => {
    const h = harness();
    h.controller.refresh();
    await loadInitial(h, TIMELINE_PAGE_SIZE);

    h.controller.loadMore();
    h.pending[1]?.resolve(failure("Couldn't reach the Axum API."));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.items).toHaveLength(TIMELINE_PAGE_SIZE);
    expect(state.tag === "ready" && state.loadingMore).toBe(false);
    expect(state.tag === "ready" && state.error).toBe("Couldn't reach the Axum API.");

    h.controller.loadMore();
    expect(h.requests[2]).toEqual({ beforeId: TIMELINE_PAGE_SIZE, limit: TIMELINE_PAGE_SIZE });
  });

  it("drops a stale page that resolves after a newer refresh", async () => {
    const h = harness();
    h.controller.refresh();
    await loadInitial(h, TIMELINE_PAGE_SIZE);

    h.controller.loadMore();
    h.controller.refresh();
    expect(h.requests[2]).toEqual({ beforeId: null, limit: TIMELINE_PAGE_SIZE });

    h.pending[2]?.resolve(success([item(1)]));
    await settle();
    h.pending[1]?.resolve(success([item(500)]));
    await settle();

    expect(h.events).toContainEqual({ type: "superseded", of: "more" });
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.items.map((entry) => entry.id)).toEqual([1]);
    expect(state.tag === "ready" && state.loadingMore).toBe(false);
  });

  it("shows the refresh spinner while keeping the list during a pull-to-refresh", async () => {
    const h = harness();
    h.controller.refresh();
    await loadInitial(h, 2);

    h.controller.refresh();
    const refreshingState = replay(h.events, initial);
    expect(refreshingState.tag === "ready" && refreshingState.items).toHaveLength(2);
    expect(refreshingState.tag === "ready" && refreshingState.refreshing).toBe(true);

    h.pending[1]?.resolve(success([]));
    await settle();
    const refreshed = replay(h.events, initial);
    expect(refreshed.tag === "ready" && refreshed.items).toEqual([]);
  });

  it("keeps the list and reports an inline error when a refresh fails", async () => {
    const h = harness();
    h.controller.refresh();
    await loadInitial(h, 2);

    h.controller.refresh();
    h.pending[1]?.resolve(failure("Couldn't reach the Axum API."));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.items).toHaveLength(2);
    expect(state.tag === "ready" && state.refreshing).toBe(false);
    expect(state.tag === "ready" && state.error).toBe("Couldn't reach the Axum API.");
  });

  it("drops a superseded initial load response", async () => {
    const h = harness();
    h.controller.refresh();
    h.controller.refresh();

    h.pending[1]?.resolve(success([item(1)]));
    h.pending[0]?.resolve(success([item(999)]));
    await settle();

    const succeeded = h.events.filter((event) => event.type === "loadSucceeded");
    expect(succeeded).toHaveLength(1);
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.items.map((entry) => entry.id)).toEqual([1]);
  });

  it("cancelling drops in-flight responses", async () => {
    const h = harness();
    h.controller.refresh();
    h.controller.cancel();

    h.pending[0]?.resolve(success([item(1)]));
    await settle();

    expect(h.events.map((event) => event.type)).toEqual(["loadStarted", "superseded"]);
  });
});
