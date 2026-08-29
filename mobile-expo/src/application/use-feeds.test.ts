import { describe, expect, it } from "vitest";

import type { Feed } from "../domain/rss";
import { failure, success, type Result } from "../domain/result";
import {
  createFeedsController,
  reduce,
  type FeedsEvent,
  type FeedsState,
} from "./use-feeds";

function feed(id: number, url: string): Feed {
  return { id, url, title: `Feed ${id}` };
}

/** A scripted fake of the rss feed adapter surface with manually resolved responses. */
function fakeApi() {
  const calls: Array<
    | { readonly kind: "list" }
    | { readonly kind: "create"; readonly url: string }
    | { readonly kind: "delete"; readonly id: number }
  > = [];
  const listPending: Array<(result: Result<readonly Feed[]>) => void> = [];
  const createPending: Array<{
    readonly url: string;
    resolve: (result: Result<Feed>) => void;
  }> = [];
  const deletePending: Array<{
    readonly id: number;
    resolve: (result: Result<void>) => void;
  }> = [];
  const api = {
    compactTimeline: () => {
      throw new Error("timeline not used by the feeds machine");
    },
    listFeeds: () => {
      calls.push({ kind: "list" });
      return new Promise<Result<readonly Feed[]>>((resolve) => {
        listPending.push(resolve);
      });
    },
    createFeed: (url: string) => {
      calls.push({ kind: "create", url });
      return new Promise<Result<Feed>>((resolve) => {
        createPending.push({ url, resolve });
      });
    },
    deleteFeed: (id: number) => {
      calls.push({ kind: "delete", id });
      return new Promise<Result<void>>((resolve) => {
        deletePending.push({ id, resolve });
      });
    },
  };
  return { api, calls, listPending, createPending, deletePending };
}

function harness(
  initialMirrors: { readonly input: string; readonly undoable: readonly Feed[] } = {
    input: "",
    undoable: [],
  },
) {
  const { api, calls, listPending, createPending, deletePending } = fakeApi();
  const events: FeedsEvent[] = [];
  let timelineRefreshes = 0;
  const controller = createFeedsController({
    api,
    emit: (event) => events.push(event),
    refreshTimeline: () => {
      timelineRefreshes += 1;
    },
    initial: initialMirrors,
  });
  return { controller, calls, listPending, createPending, deletePending, events, refreshed: () => timelineRefreshes };
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function replay(events: ReadonlyArray<FeedsEvent>, initial: FeedsState): FeedsState {
  return events.reduce((state, event) => reduce(state, event), initial);
}

const initial: FeedsState = { tag: "loading" };

const FEEDS = [feed(1, "https://a.test/feed.xml"), feed(2, "https://b.test/rss.xml")];

describe("feeds state machine", () => {
  it("loads the current feeds into a ready list", async () => {
    const h = harness();
    h.controller.refresh();
    expect(h.calls).toEqual([{ kind: "list" }]);

    h.listPending[0]?.(success(FEEDS));
    await settle();

    expect(h.events.map((event) => event.type)).toEqual(["loadStarted", "loadSucceeded"]);
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.feeds).toEqual(FEEDS);
    expect(state.tag === "ready" && state.undoable).toEqual([]);
    expect(state.tag === "ready" && state.error).toBeNull();
  });

  it("surfaces a failed feed load as a retryable error", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(failure("Couldn't reach the Axum API."));
    await settle();

    expect(replay(h.events, initial)).toEqual({
      tag: "error",
      message: "Couldn't reach the Axum API.",
    });

    h.controller.refresh();
    h.listPending[1]?.(success(FEEDS));
    await settle();
    expect(replay(h.events, initial).tag).toBe("ready");
  });

  it("keeps the list and reports an inline error when a refresh fails", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.refresh();
    h.listPending[1]?.(failure("Couldn't reach the Axum API."));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.feeds).toEqual(FEEDS);
    expect(state.tag === "ready" && state.error).toBe("Couldn't reach the Axum API.");
  });

  it("rejects an invalid URL client-side without sending any request", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.setInput("not a url");
    h.controller.create();
    await settle();

    expect(h.calls).toEqual([{ kind: "list" }]);
    const state = replay(h.events, initial);
    expect(
      state.tag === "ready" &&
        state.error === "Enter a valid feed URL starting with http:// or https://.",
    ).toBe(true);
    expect(state.tag === "ready" && state.input).toBe("not a url");
    expect(state.tag === "ready" && state.busy).toBe(false);
  });

  it("rejects an empty URL before any request is sent", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.setInput("   ");
    h.controller.create();
    await settle();

    expect(h.calls).toEqual([{ kind: "list" }]);
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.error).toBe("Enter a feed URL to add.");
  });

  it("creates a feed from the trimmed URL and refreshes the list and timeline", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.setInput("  https://c.test/feed.xml  ");
    h.controller.create();
    await settle();

    expect(h.calls[1]).toEqual({ kind: "create", url: "https://c.test/feed.xml" });
    h.createPending[0]?.resolve(success(feed(3, "https://c.test/feed.xml")));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.input).toBe("");
    expect(state.tag === "ready" && state.error).toBeNull();
    // The create is followed by a feed list reload and a timeline refresh.
    expect(h.calls[2]).toEqual({ kind: "list" });
    expect(h.refreshed()).toBe(1);

    h.listPending[1]?.(success([...FEEDS, feed(3, "https://c.test/feed.xml")]));
    await settle();
    const loaded = replay(h.events, initial);
    expect(loaded.tag === "ready" && loaded.feeds).toHaveLength(3);
    expect(loaded.tag === "ready" && loaded.busy).toBe(false);
  });

  it("surfaces a create failure inline without losing the entered URL", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.setInput("https://c.test/feed.xml");
    h.controller.create();
    await settle();
    h.createPending[0]?.resolve(failure("Feed with this URL already exists"));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.error).toBe("Feed with this URL already exists");
    expect(state.tag === "ready" && state.input).toBe("https://c.test/feed.xml");
    expect(state.tag === "ready" && state.busy).toBe(false);
    // No list reload or timeline refresh after a failed create.
    expect(h.calls).toEqual([{ kind: "list" }, { kind: "create", url: "https://c.test/feed.xml" }]);
    expect(h.refreshed()).toBe(0);
  });

  it("clears the undo batch when a feed is created", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.remove(FEEDS[0]!);
    h.deletePending[0]?.resolve(success(undefined));
    await settle();
    let state = replay(h.events, initial);
    expect(state.tag === "ready" && state.undoable).toEqual([FEEDS[0]]);

    h.controller.setInput("https://c.test/feed.xml");
    h.controller.create();
    await settle();
    h.createPending[0]?.resolve(success(feed(3, "https://c.test/feed.xml")));
    await settle();
    h.listPending[1]?.(success(FEEDS));
    await settle();

    state = replay(h.events, initial);
    expect(state.tag === "ready" && state.undoable).toEqual([]);
  });

  it("removes a deleted feed immediately and moves it to the undo batch", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.remove(FEEDS[1]!);
    await settle();

    let state = replay(h.events, initial);
    expect(state.tag === "ready" && state.feeds).toEqual([FEEDS[0]]);
    expect(state.tag === "ready" && state.undoable).toEqual([FEEDS[1]]);

    h.deletePending[0]?.resolve(success(undefined));
    await settle();
    state = replay(h.events, initial);
    expect(state.tag === "ready" && state.busy).toBe(false);
    expect(h.refreshed()).toBe(1);
  });

  it("restores the feed and surfaces the error when a delete fails", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.remove(FEEDS[1]!);
    h.deletePending[0]?.resolve(failure("Feed 2 not found."));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.feeds).toEqual(FEEDS);
    expect(state.tag === "ready" && state.undoable).toEqual([]);
    expect(state.tag === "ready" && state.error).toBe("Feed 2 not found.");
    expect(h.refreshed()).toBe(0);
  });

  it("re-adds the whole undo batch on undo and refreshes list and timeline", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.remove(FEEDS[0]!);
    await settle();
    h.deletePending[0]?.resolve(success(undefined));
    await settle();
    h.controller.remove(FEEDS[1]!);
    await settle();
    h.deletePending[1]?.resolve(success(undefined));
    await settle();
    expect(h.refreshed()).toBe(2);

    h.controller.undo();
    await settle();
    // One batched undo re-adds every feed deleted since the last undo or creation.
    expect(h.calls).toEqual([
      { kind: "list" },
      { kind: "delete", id: 1 },
      { kind: "delete", id: 2 },
      { kind: "create", url: "https://a.test/feed.xml" },
      { kind: "create", url: "https://b.test/rss.xml" },
    ]);

    h.createPending[0]?.resolve(success(feed(3, "https://a.test/feed.xml")));
    h.createPending[1]?.resolve(success(feed(4, "https://b.test/rss.xml")));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.undoable).toEqual([]);
    expect(state.tag === "ready" && state.error).toBeNull();
    expect(h.refreshed()).toBe(3);
    expect(h.calls.at(-1)).toEqual({ kind: "list" });
  });

  it("keeps failed re-adds undoable when an undo partially fails", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.remove(FEEDS[0]!);
    h.deletePending[0]?.resolve(success(undefined));
    await settle();
    h.controller.remove(FEEDS[1]!);
    h.deletePending[1]?.resolve(success(undefined));
    await settle();

    h.controller.undo();
    await settle();
    h.createPending[0]?.resolve(success(feed(3, "https://a.test/feed.xml")));
    h.createPending[1]?.resolve(failure("Feed with this URL already exists"));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.undoable).toEqual([FEEDS[1]]);
    expect(state.tag === "ready" && state.error).toBe("Feed with this URL already exists");
    expect(h.refreshed()).toBe(3);
  });

  it("ignores undo when nothing has been deleted", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.undo();
    await settle();

    expect(h.calls).toEqual([{ kind: "list" }]);
    expect(h.events.map((event) => event.type)).toEqual(["loadStarted", "loadSucceeded"]);
  });

  it("clears the inline error when the input changes", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.setInput("not a url");
    h.controller.create();
    await settle();
    let state = replay(h.events, initial);
    expect(state.tag === "ready" && state.error).not.toBeNull();

    h.controller.setInput("https://c.test/feed.xml");
    state = replay(h.events, initial);
    expect(state.tag === "ready" && state.error).toBeNull();
    expect(state.tag === "ready" && state.input).toBe("https://c.test/feed.xml");
  });

  it("keeps an in-flight deleted feed hidden when a load resolves mid-delete", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.remove(FEEDS[0]!);
    h.controller.refresh();
    // The load still sees the feed server-side; the optimistic removal stands.
    h.listPending[1]?.(success(FEEDS));
    await settle();

    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.feeds).toEqual([FEEDS[1]]);
    expect(state.tag === "ready" && state.undoable).toEqual([FEEDS[0]]);
    expect(state.tag === "ready" && state.busy).toBe(true);

    h.deletePending[0]?.resolve(success(undefined));
    await settle();
    const settled = replay(h.events, initial);
    expect(settled.tag === "ready" && settled.feeds).toEqual([FEEDS[1]]);
    expect(settled.tag === "ready" && settled.undoable).toEqual([FEEDS[0]]);
  });

  it("drops a pre-delete load snapshot that resolves after the delete commits", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    // This load's snapshot predates the delete; it must not reconcile the
    // just-deleted feed back into the list or wipe the undo batch.
    h.controller.refresh();
    h.controller.remove(FEEDS[0]!);
    h.deletePending[0]?.resolve(success(undefined));
    await settle();
    h.listPending[1]?.(success(FEEDS));
    await settle();

    expect(h.events.filter((event) => event.type === "loadSucceeded")).toHaveLength(1);
    const state = replay(h.events, initial);
    expect(state.tag === "ready" && state.feeds).toEqual([FEEDS[1]]);
    expect(state.tag === "ready" && state.undoable).toEqual([FEEDS[0]]);
  });

  it("rehydrates the undo batch and input into a recreated controller", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.setInput("https://c.test/feed.xml");
    h.controller.remove(FEEDS[0]!);
    // The api identity changed; the old controller's cleanup clears its flag.
    h.controller.cancel();

    const carried = replay(h.events, initial);
    expect(carried.tag === "ready" && carried.busy).toBe(false);
    expect(carried.tag === "ready" && carried.undoable).toEqual([FEEDS[0]]);

    const next = harness({
      input: carried.tag === "ready" ? carried.input : "",
      undoable: carried.tag === "ready" ? carried.undoable : [],
    });
    next.controller.undo();
    await settle();
    expect(next.calls[0]).toEqual({ kind: "create", url: "https://a.test/feed.xml" });

    next.createPending[0]?.resolve(success(feed(3, "https://a.test/feed.xml")));
    await settle();
    expect(next.refreshed()).toBe(1);

    // The carried input survives without re-typing: create uses it directly.
    const recreated = harness({ input: "https://c.test/feed.xml", undoable: [] });
    recreated.controller.create();
    await settle();
    expect(recreated.calls[0]).toEqual({ kind: "create", url: "https://c.test/feed.xml" });
  });

  it("drops a batch entry the server already restored instead of retrying it forever", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success(FEEDS));
    await settle();

    h.controller.remove(FEEDS[0]!);
    h.deletePending[0]?.resolve(success(undefined));
    await settle();

    h.controller.undo();
    await settle();
    // The re-add committed server-side but its response was lost.
    h.createPending[0]?.resolve(failure("Couldn't reach the Axum API."));
    await settle();

    let state = replay(h.events, initial);
    expect(state.tag === "ready" && state.undoable).toEqual([FEEDS[0]]);
    expect(state.tag === "ready" && state.error).toBe("Couldn't reach the Axum API.");

    // The follow-up list load shows the feed was restored; the batch clears.
    h.listPending[1]?.(success([FEEDS[0]!, FEEDS[1]!]));
    await settle();
    state = replay(h.events, initial);
    expect(state.tag === "ready" && state.undoable).toEqual([]);
    expect(state.tag === "ready" && state.feeds).toEqual([FEEDS[0], FEEDS[1]]);
  });
});
