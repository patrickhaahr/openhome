import { useEffect, useMemo, useReducer, useRef } from "react";

import { parseFeedUrl, type Feed } from "../domain/rss";
import type { RssApi } from "../infrastructure/open-home-api";

/** The user-visible state of the inline Feed manager. */
export type FeedsState =
  | { readonly tag: "loading" }
  | { readonly tag: "error"; readonly message: string }
  | {
      readonly tag: "ready";
      readonly feeds: readonly Feed[];
      /** Feeds deleted since the last undo or creation; undo re-adds the batch. */
      readonly undoable: readonly Feed[];
      /** A create, delete, or undo is in flight. */
      readonly busy: boolean;
      readonly error: string | null;
      readonly input: string;
    };

/** User actions accepted by the Feed manager. */
export type FeedsActions = {
  readonly refresh: () => void;
  readonly setInput: (value: string) => void;
  readonly create: () => void;
  readonly remove: (feed: Feed) => void;
  readonly undo: () => void;
};

export type FeedsEvent =
  | { readonly type: "loadStarted" }
  | { readonly type: "loadSucceeded"; readonly feeds: readonly Feed[] }
  | { readonly type: "loadFailed"; readonly message: string }
  | { readonly type: "inputChanged"; readonly value: string }
  /** The pasted URL was rejected on the client before any request was sent. */
  | { readonly type: "inputRejected"; readonly message: string }
  | { readonly type: "createStarted" }
  | { readonly type: "createSucceeded" }
  | { readonly type: "createFailed"; readonly message: string }
  | { readonly type: "deleteStarted"; readonly feed: Feed }
  | { readonly type: "deleteSucceeded"; readonly id: number }
  | { readonly type: "deleteFailed"; readonly feed: Feed; readonly message: string }
  | { readonly type: "undoStarted" }
  | {
      readonly type: "undoFinished";
      /** Feeds that could not be re-added stay undoable for a retry. */
      readonly undoable: readonly Feed[];
      readonly error: string | null;
    }
  /** A cancelled controller's in-flight mutation resolved; its flag must clear. */
  | { readonly type: "superseded"; readonly of: "mutation" };

/** Drives the Feed manager against the Axum API adapter, emitting events. */
export function createFeedsController(deps: {
  readonly api: Pick<RssApi, "listFeeds" | "createFeed" | "deleteFeed">;
  readonly emit: (event: FeedsEvent) => void;
  /** Refreshes the Compact Timeline; feed mutations change what it shows. */
  readonly refreshTimeline: () => void;
  /** Reducer state carried over from a previous controller for this machine. */
  readonly initial: { readonly input: string; readonly undoable: readonly Feed[] };
}) {
  let loadToken = 0;
  let mutateToken = 0;
  let busy = false;
  let input = deps.initial.input;
  let undoable = deps.initial.undoable;

  async function load(): Promise<void> {
    const current = ++loadToken;
    deps.emit({ type: "loadStarted" });
    const result = await deps.api.listFeeds();
    if (loadToken !== current) {
      return;
    }
    deps.emit(
      result.ok
        ? { type: "loadSucceeded", feeds: result.value }
        : { type: "loadFailed", message: result.error },
    );
  }

  async function create(): Promise<void> {
    if (busy) {
      return;
    }
    const validation = parseFeedUrl(input);
    if (!validation.ok) {
      deps.emit({ type: "inputRejected", message: validation.error });
      return;
    }
    busy = true;
    const current = ++mutateToken;
    // A load in flight since before this mutation carries a stale snapshot;
    // drop it so it can neither hide the mutation nor mis-reconcile the batch.
    loadToken += 1;
    deps.emit({ type: "createStarted" });
    const result = await deps.api.createFeed(validation.value);
    if (mutateToken !== current) {
      busy = false;
      deps.emit({ type: "superseded", of: "mutation" });
      return;
    }
    busy = false;
    if (!result.ok) {
      deps.emit({ type: "createFailed", message: result.error });
      return;
    }
    input = "";
    undoable = [];
    deps.emit({ type: "createSucceeded" });
    deps.refreshTimeline();
    void load();
  }

  async function remove(feed: Feed): Promise<void> {
    if (busy) {
      return;
    }
    busy = true;
    const current = ++mutateToken;
    loadToken += 1;
    undoable = [...undoable, feed];
    deps.emit({ type: "deleteStarted", feed });
    const result = await deps.api.deleteFeed(feed.id);
    if (mutateToken !== current) {
      busy = false;
      deps.emit({ type: "superseded", of: "mutation" });
      return;
    }
    busy = false;
    if (!result.ok) {
      undoable = undoable.filter((entry) => entry.id !== feed.id);
      deps.emit({ type: "deleteFailed", feed, message: result.error });
      return;
    }
    deps.emit({ type: "deleteSucceeded", id: feed.id });
    deps.refreshTimeline();
  }

  async function undo(): Promise<void> {
    if (busy || undoable.length === 0) {
      return;
    }
    busy = true;
    const current = ++mutateToken;
    loadToken += 1;
    const batch = undoable;
    deps.emit({ type: "undoStarted" });
    const results = await Promise.all(batch.map((feed) => deps.api.createFeed(feed.url)));
    if (mutateToken !== current) {
      busy = false;
      deps.emit({ type: "superseded", of: "mutation" });
      return;
    }
    busy = false;
    const failed: Feed[] = [];
    let error: string | null = null;
    results.forEach((result, index) => {
      const feed = batch[index];
      if (feed !== undefined && !result.ok) {
        failed.push(feed);
        error ??= result.error;
      }
    });
    undoable = failed;
    deps.emit({ type: "undoFinished", undoable: failed, error });
    if (failed.length < batch.length) {
      deps.refreshTimeline();
    }
    // Always reconcile against the server: a "failed" re-add may have committed
    // with its response lost, and the next load drops it from the batch.
    void load();
  }

  return {
    refresh(): void {
      void load();
    },
    setInput(value: string): void {
      input = value;
      deps.emit({ type: "inputChanged", value });
    },
    create(): void {
      void create();
    },
    remove(feed: Feed): void {
      void remove(feed);
    },
    undo(): void {
      void undo();
    },
    cancel(): void {
      loadToken += 1;
      mutateToken += 1;
      if (busy) {
        busy = false;
        deps.emit({ type: "superseded", of: "mutation" });
      }
    },
  };
}

/** Coordinate Feed manager state for the Server Tab UI. */
export function useFeeds(
  api: RssApi | null,
  refreshTimeline: () => void,
): readonly [FeedsState, FeedsActions] {
  const [state, dispatch] = useReducer(reduce, { tag: "loading" });
  const controller = useRef<ReturnType<typeof createFeedsController> | null>(null);

  useEffect(() => {
    if (api === null) {
      controller.current = null;
      return;
    }
    const current = createFeedsController({
      api,
      emit: dispatch,
      refreshTimeline,
      initial: {
        input: state.tag === "ready" ? state.input : "",
        undoable: state.tag === "ready" ? state.undoable : [],
      },
    });
    controller.current = current;
    current.refresh();
    return () => current.cancel();
    // state is read only when the controller is (re)created; recreating on every
    // state change would restart in-flight operations.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, refreshTimeline]);

  return [
    state,
    useMemo<FeedsActions>(
      () => ({
        refresh: () => controller.current?.refresh(),
        setInput: (value) => controller.current?.setInput(value),
        create: () => controller.current?.create(),
        remove: (feed) => controller.current?.remove(feed),
        undo: () => controller.current?.undo(),
      }),
      [],
    ),
  ];
}

/** Apply a machine event to Feed manager state. */
export function reduce(state: FeedsState, event: FeedsEvent): FeedsState {
  switch (event.type) {
    case "loadStarted":
      return state.tag === "ready" ? state : { tag: "loading" };
    case "loadSucceeded": {
      const ready = state.tag === "ready" ? state : null;
      const merged = mergeLoaded(ready?.undoable ?? [], event.feeds, ready?.busy ?? false);
      return {
        tag: "ready",
        feeds: merged.feeds,
        undoable: merged.undoable,
        busy: ready?.busy ?? false,
        error: null,
        input: ready?.input ?? "",
      };
    }
    case "loadFailed":
      return state.tag === "ready"
        ? { ...state, error: event.message }
        : { tag: "error", message: event.message };
    case "inputChanged":
      return state.tag === "ready" ? { ...state, input: event.value, error: null } : state;
    case "inputRejected":
      return state.tag === "ready" ? { ...state, error: event.message } : state;
    case "createStarted":
      return state.tag === "ready" ? { ...state, busy: true, error: null } : state;
    case "createSucceeded":
      return state.tag === "ready"
        ? { ...state, busy: false, input: "", undoable: [], error: null }
        : state;
    case "createFailed":
      return state.tag === "ready" ? { ...state, busy: false, error: event.message } : state;
    case "deleteStarted":
      return state.tag === "ready"
        ? {
            ...state,
            feeds: state.feeds.filter((entry) => entry.id !== event.feed.id),
            undoable: [...state.undoable, event.feed],
            busy: true,
            error: null,
          }
        : state;
    case "deleteSucceeded":
      return state.tag === "ready" ? { ...state, busy: false } : state;
    case "deleteFailed":
      return state.tag === "ready"
        ? {
            ...state,
            feeds: [...state.feeds, event.feed],
            undoable: state.undoable.filter((entry) => entry.id !== event.feed.id),
            busy: false,
            error: event.message,
          }
        : state;
    case "undoStarted":
      return state.tag === "ready" ? { ...state, busy: true, error: null } : state;
    case "undoFinished":
      return state.tag === "ready"
        ? { ...state, busy: false, undoable: event.undoable, error: event.error }
        : state;
    case "superseded":
      return state.tag === "ready" ? { ...state, busy: false } : state;
  }
}

/**
 * Merge a freshly loaded feed list with the undo batch. While a delete is in
 * flight the list hides its optimistically removed feed; once no mutation is
 * in flight, batch entries the server already lists were restored anyway
 * (their re-add response was lost) and leave the batch instead of retrying
 * into a permanent duplicate-URL error.
 */
function mergeLoaded(undoable: readonly Feed[], loaded: readonly Feed[], busy: boolean) {
  const nextUndoable = busy
    ? undoable
    : undoable.filter((entry) => !loaded.some((feed) => feed.url === entry.url));
  return {
    feeds: loaded.filter((feed) => !nextUndoable.some((entry) => entry.url === feed.url)),
    undoable: nextUndoable,
  };
}
