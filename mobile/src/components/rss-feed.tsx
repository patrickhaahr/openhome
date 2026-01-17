import type { Component } from "solid-js";
import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getTimelineCompact, getFeeds, createFeed, deleteFeed, type TimelineCompactItem, type FeedItem } from "../api/rss";
import { Button } from "./ui/button";
import { ScrollArea } from "./ui/scroll-area";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "./ui/card";

const RssFeed: Component = () => {
  const [timelineItems, setTimelineItems] = createSignal<TimelineCompactItem[]>([]);
  const [feeds, setFeeds] = createSignal<FeedItem[]>([]);
  const [beforeId, setBeforeId] = createSignal<number | null>(null);
  const [isLoadingMore, setIsLoadingMore] = createSignal(false);
  const [hasMore, setHasMore] = createSignal(true);
  const [statusMessage, setStatusMessage] = createSignal("");
  const [newFeedUrl, setNewFeedUrl] = createSignal("");
  let statusTimer: number | undefined;

  const setStatus = (msg: string) => {
    setStatusMessage(msg);
    if (statusTimer !== undefined) {
      window.clearTimeout(statusTimer);
    }
    statusTimer = window.setTimeout(() => setStatusMessage(""), 3000);
  };

  const loadFeeds = async () => {
    try {
      const feedList = await getFeeds();
      setFeeds(feedList);
    } catch (e) {
      console.error("Failed to fetch feeds:", e);
    }
  };

  const loadTimeline = async (reset: boolean = false) => {
    if (isLoadingMore()) {
      return;
    }

    setIsLoadingMore(true);
    try {
      const items = await getTimelineCompact({
        limit: 50,
        beforeId: reset ? undefined : beforeId(),
      });

      if (reset) {
        setTimelineItems(items);
        setBeforeId(items.length > 0 ? items[items.length - 1].id : null);
        setHasMore(items.length >= 50);
      } else {
        setTimelineItems((prev) => [...prev, ...items]);
        if (items.length > 0) {
          setBeforeId(items[items.length - 1].id);
        }
        setHasMore(items.length >= 50);
      }
    } catch (e) {
      console.error("Failed to fetch timeline:", e);
      setStatusMessage("Failed to load timeline");
    } finally {
      setIsLoadingMore(false);
    }
  };

  const handleAddFeed = async () => {
    const url = newFeedUrl().trim();
    if (!url) {
      setStatus("URL is required");
      return;
    }

    try {
      await createFeed(url);
      setNewFeedUrl("");
      setStatus("Feed added successfully");
      await loadFeeds();
      await loadTimeline(true);
    } catch (e) {
      console.error("Failed to create feed:", e);
      setStatus("Failed to add feed");
    }
  };

  const handleDeleteFeed = async (id: number, title: string) => {
    const confirmed = confirm(`Delete feed "${title}"?`);
    if (!confirmed) {
      return;
    }

    try {
      await deleteFeed(id);
      setStatus("Feed deleted successfully");
      await loadFeeds();
      await loadTimeline(true);
    } catch (e) {
      console.error("Failed to delete feed:", e);
      setStatus("Failed to delete feed");
    }
  };

  const openHttpUrl = async (raw: string) => {
    let url: URL;
    try {
      url = new URL(raw);
    } catch {
      setStatus("Invalid link");
      return;
    }
    if (url.protocol !== "https:") {
      setStatus(`Blocked unsupported link scheme: ${url.protocol}`);
      return;
    }
    await openUrl(url.toString());
  };

  const handleItemClick = async (link: string) => {
    try {
      await openHttpUrl(link);
    } catch (e) {
      console.error("Failed to open link:", e);
      setStatus("Failed to open link");
    }
  };

  const handleScroll = (e: Event & { currentTarget: HTMLDivElement }) => {
    const target = e.currentTarget;
    const isNearBottom =
      target.scrollTop + target.clientHeight >= target.scrollHeight - 8;

    if (isNearBottom && hasMore() && !isLoadingMore()) {
      loadTimeline(false);
    }
  };

  onMount(() => {
    loadFeeds();
    loadTimeline(true);
  });

  onCleanup(() => {
    if (statusTimer !== undefined) {
      window.clearTimeout(statusTimer);
    }
  });

  return (
    <div class="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Add RSS Feed</CardTitle>
          <CardDescription>Add a new RSS feed URL</CardDescription>
        </CardHeader>
        <CardContent>
          <div class="flex flex-col gap-3">
            <input
              type="text"
              value={newFeedUrl()}
              onInput={(e) => setNewFeedUrl(e.currentTarget.value)}
              placeholder="https://example.com/feed.xml"
              class="w-full rounded-lg border border-border bg-bg-tertiary px-4 py-3 text-text-primary placeholder-text-muted transition-colors focus:border-accent focus:outline-none"
            />
            <div class="flex items-center justify-between gap-3">
              <Button onClick={handleAddFeed} class="flex-1">
                Add Feed
              </Button>
              <Show when={statusMessage()}>
                <span
                  class={`text-sm ${
                    statusMessage().includes("Failed")
                      ? "text-error"
                      : "text-success"
                  }`}
                >
                  {statusMessage()}
                </span>
              </Show>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Feeds</CardTitle>
          <CardDescription>Manage your RSS feeds</CardDescription>
        </CardHeader>
        <CardContent>
          <Show
            when={feeds().length > 0}
            fallback={
              <p class="text-sm text-text-muted">No feeds configured</p>
            }
          >
            <div class="space-y-2">
              <For each={feeds()}>
                {(feed) => (
                  <div class="flex items-center justify-between rounded-lg bg-bg-tertiary px-4 py-3">
                    <span class="text-sm font-medium text-text-primary">
                      {feed.title ?? "Untitled Feed"}
                    </span>
                    <Button
                      variant="destructive"
                      size="sm"
                      onClick={() => handleDeleteFeed(feed.id, feed.title ?? "Untitled Feed")}
                    >
                      Delete
                    </Button>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Timeline</CardTitle>
          <CardDescription>
            Latest items from your RSS feeds
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Show
            when={timelineItems().length > 0}
            fallback={
              <p class="text-sm text-text-muted">
                No items available. Add feeds to populate the timeline.
              </p>
            }
          >
            <ScrollArea class="max-h-96 space-y-3" onScroll={handleScroll}>
              <For each={timelineItems()}>
                {(item) => (
                  <Card
                    onClick={() => handleItemClick(item.link)}
                    class="cursor-pointer transition-colors hover:bg-bg-tertiary"
                  >
                    <CardHeader class="p-4 pb-2">
                      <CardTitle class="text-base">{item.title}</CardTitle>
                    </CardHeader>
                    <Show when={item.description}>
                      <CardContent class="p-4 pt-0">
                        <CardDescription class="text-xs">
                          {item.description}
                        </CardDescription>
                      </CardContent>
                    </Show>
                  </Card>
                )}
              </For>
              <Show when={isLoadingMore()}>
                <p class="py-4 text-center text-sm text-text-muted">
                  Loading more...
                </p>
              </Show>
              <Show when={!hasMore() && timelineItems().length > 0}>
                <p class="py-4 text-center text-sm text-text-muted">
                  No more items
                </p>
              </Show>
            </ScrollArea>
          </Show>
        </CardContent>
      </Card>
    </div>
  );
};

export default RssFeed;
