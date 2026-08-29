import { useEffect, useMemo, useReducer, useRef } from "react";

import type { TimelineItem } from "../domain/rss";
import type { RssApi } from "../infrastructure/open-home-api";

/** The user-visible state of the Compact Timeline. */
export type TimelineState =
  | { readonly tag: "loading" }
  | { readonly tag: "error"; readonly message: string }
  | {
      readonly tag: "ready";
      readonly items: readonly TimelineItem[];
      /** A refresh is in flight; the previous items stay visible. */
      readonly refreshing: boolean;
      /** The next page load is in flight. */
      readonly loadingMore: boolean;
      /** True while older items may still exist beyond the loaded pages. */
      readonly hasMore: boolean;
      readonly error: string | null;
    };

/** User actions accepted by the Compact Timeline. */
export type TimelineActions = {
  readonly refresh: () => void;
  readonly loadMore: () => void;
};

export type TimelineEvent =
  | { readonly type: "loadStarted" }
  | { readonly type: "loadSucceeded"; readonly items: readonly TimelineItem[] }
  | { readonly type: "loadFailed"; readonly message: string }
  | { readonly type: "moreStarted" }
  | { readonly type: "moreSucceeded"; readonly page: readonly TimelineItem[] }
  | { readonly type: "moreFailed"; readonly message: string }
  /** A superseded operation resolved; its result is dropped but its own flag must clear. */
  | { readonly type: "superseded"; readonly of: "refresh" | "more" };

/** Page size requested from the timeline endpoint; a short page ends the list. */
export const TIMELINE_PAGE_SIZE = 50;

/** Drives the timeline machine against the Axum API adapter, emitting events. */
export function createTimelineController(deps: {
  readonly api: Pick<RssApi, "compactTimeline">;
  readonly emit: (event: TimelineEvent) => void;
}) {
  let refreshToken = 0;
  let moreToken = 0;
  /** The id pagination resumes after: the last item of the newest loaded page. */
  let cursor: number | null = null;
  let hasMore = false;
  let loadingMore = false;

  async function fetchFirstPage(): Promise<void> {
    const current = ++refreshToken;
    moreToken += 1;
    deps.emit({ type: "loadStarted" });
    const result = await deps.api.compactTimeline(null, TIMELINE_PAGE_SIZE);
    if (refreshToken !== current) {
      deps.emit({ type: "superseded", of: "refresh" });
      return;
    }
    if (!result.ok) {
      deps.emit({ type: "loadFailed", message: result.error });
      return;
    }
    cursor = lastId(result.value);
    hasMore = result.value.length >= TIMELINE_PAGE_SIZE;
    deps.emit({ type: "loadSucceeded", items: result.value });
  }

  async function fetchNextPage(): Promise<void> {
    if (loadingMore || cursor === null || !hasMore) {
      return;
    }
    loadingMore = true;
    const current = ++moreToken;
    deps.emit({ type: "moreStarted" });
    const result = await deps.api.compactTimeline(cursor, TIMELINE_PAGE_SIZE);
    loadingMore = false;
    if (moreToken !== current) {
      deps.emit({ type: "superseded", of: "more" });
      return;
    }
    if (!result.ok) {
      deps.emit({ type: "moreFailed", message: result.error });
      return;
    }
    if (result.value.length > 0) {
      cursor = lastId(result.value);
    }
    hasMore = result.value.length >= TIMELINE_PAGE_SIZE;
    deps.emit({ type: "moreSucceeded", page: result.value });
  }

  return {
    refresh(): void {
      void fetchFirstPage();
    },
    loadMore(): void {
      void fetchNextPage();
    },
    cancel(): void {
      refreshToken += 1;
      moreToken += 1;
    },
  };
}

/** Coordinate Compact Timeline state for the Server Tab UI. */
export function useTimeline(api: RssApi | null): readonly [TimelineState, TimelineActions] {
  const [state, dispatch] = useReducer(reduce, { tag: "loading" });
  const controller = useRef<ReturnType<typeof createTimelineController> | null>(null);

  useEffect(() => {
    if (api === null) {
      controller.current = null;
      return;
    }
    const current = createTimelineController({ api, emit: dispatch });
    controller.current = current;
    current.refresh();
    return () => current.cancel();
  }, [api]);

  return [
    state,
    useMemo<TimelineActions>(
      () => ({
        refresh: () => controller.current?.refresh(),
        loadMore: () => controller.current?.loadMore(),
      }),
      [],
    ),
  ];
}

/** Apply a machine event to Compact Timeline state. */
export function reduce(state: TimelineState, event: TimelineEvent): TimelineState {
  switch (event.type) {
    case "loadStarted":
      return state.tag === "ready" ? { ...state, refreshing: true } : { tag: "loading" };
    case "loadSucceeded":
      return state.tag === "ready"
        ? {
            ...state,
            items: event.items,
            hasMore: event.items.length >= TIMELINE_PAGE_SIZE,
            refreshing: false,
            error: null,
          }
        : {
            tag: "ready",
            items: event.items,
            refreshing: false,
            loadingMore: false,
            hasMore: event.items.length >= TIMELINE_PAGE_SIZE,
            error: null,
          };
    case "loadFailed":
      return state.tag === "ready"
        ? { ...state, refreshing: false, error: event.message }
        : { tag: "error", message: event.message };
    case "moreStarted":
      return state.tag === "ready" ? { ...state, loadingMore: true, error: null } : state;
    case "moreSucceeded":
      return state.tag === "ready"
        ? {
            ...state,
            items: appendNew(state.items, event.page),
            loadingMore: false,
            hasMore: event.page.length >= TIMELINE_PAGE_SIZE,
            error: null,
          }
        : state;
    case "moreFailed":
      return state.tag === "ready"
        ? { ...state, loadingMore: false, error: event.message }
        : state;
    case "superseded":
      if (state.tag !== "ready") {
        return state;
      }
      return event.of === "refresh"
        ? { ...state, refreshing: false }
        : { ...state, loadingMore: false };
  }
}

/** Append page items that aren't already rendered, preserving newest-first order. */
function appendNew(
  items: readonly TimelineItem[],
  page: readonly TimelineItem[],
): readonly TimelineItem[] {
  const seen = new Set(items.map((item) => item.id));
  return [...items, ...page.filter((item) => !seen.has(item.id))];
}

/** The id pagination resumes after, or null when the page is empty. */
function lastId(items: readonly TimelineItem[]): number | null {
  const last = items[items.length - 1];
  return last === undefined ? null : last.id;
}
