import { callApi } from "./index";

export interface TimelineCompactItem {
  id: number;
  title: string;
  description?: string | null;
  link: string;
}

export interface FeedItem {
  id: number;
  url: string;
  title?: string | null;
}

interface TimelineParams {
  limit?: number;
  beforeId?: number | null;
}

export async function getTimelineCompact(params: TimelineParams): Promise<TimelineCompactItem[]> {
  const query = new URLSearchParams({
    view: "compact",
    limit: String(params.limit ?? 50),
  });

  if (params.beforeId !== undefined && params.beforeId !== null) {
    query.set("before_id", String(params.beforeId));
  }

  const response = await callApi({
    path: `/api/timeline?${query.toString()}`,
    method: "GET",
  });

  if (response.status >= 200 && response.status < 300) {
    if (!Array.isArray(response.data)) {
      throw new Error("Invalid timeline response shape: expected array");
    }
    const items = response.data as unknown[];
    for (const item of items) {
      if (typeof item !== "object" || item === null || !("link" in item) || !("id" in item)) {
        throw new Error("Invalid timeline response shape: missing required fields");
      }
    }
    return response.data as TimelineCompactItem[];
  }

  throw new Error(`Failed to fetch timeline: Status ${response.status}`);
}

export async function getFeeds(): Promise<FeedItem[]> {
  const response = await callApi({
    path: "/api/feeds",
    method: "GET",
  });

  if (response.status >= 200 && response.status < 300) {
    if (!Array.isArray(response.data)) {
      throw new Error("Invalid feeds response shape: expected array");
    }
    return response.data as FeedItem[];
  }

  throw new Error(`Failed to fetch feeds: Status ${response.status}`);
}

export async function createFeed(url: string): Promise<FeedItem> {
  const response = await callApi({
    path: "/api/feeds",
    method: "POST",
    body: { url },
  });

  if (response.status >= 200 && response.status < 300) {
    if (typeof response.data !== "object" || response.data === null || !("id" in response.data)) {
      throw new Error("Invalid feed response shape: missing required fields");
    }
    return response.data as FeedItem;
  }

  throw new Error(`Failed to create feed: Status ${response.status}`);
}

export async function deleteFeed(id: number): Promise<void> {
  const response = await callApi({
    path: `/api/feeds/${id}`,
    method: "DELETE",
  });

  if (response.status === 204) {
    return;
  }

  throw new Error(`Failed to delete feed: Status ${response.status}`);
}
