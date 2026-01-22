import type { Component } from "solid-js";
import { createSignal, For, Show, onMount, onCleanup, createMemo } from "solid-js";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getTimelineCompact, getFeeds, createFeed, deleteFeed, type TimelineCompactItem, type FeedItem } from "../api/rss";
import { cn } from "@/lib/utils";
import { 
  Plus, 
  Rss, 
  Trash2, 
  ExternalLink,
  Settings2,
  X,
  Loader2,
  Inbox
} from "lucide-solid";
import {
  Dialog,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
  DialogDescription,
  ConfirmDialog,
} from "./ui/dialog";

// Strip HTML tags from description
const stripHtml = (html: string | null | undefined): string => {
  if (!html) return "";
  return html.replace(/<[^>]*>/g, "").trim();
};

// Truncate text elegantly
const truncate = (text: string | null | undefined, maxLength: number): string => {
  if (!text) return "";
  const stripped = stripHtml(text);
  if (stripped.length <= maxLength) return stripped;
  return stripped.slice(0, maxLength).trim() + "...";
};

const RssFeed: Component = () => {
  const [timelineItems, setTimelineItems] = createSignal<TimelineCompactItem[]>([]);
  const [feeds, setFeeds] = createSignal<FeedItem[]>([]);
  const [beforeId, setBeforeId] = createSignal<number | null>(null);
  const [isLoadingMore, setIsLoadingMore] = createSignal(false);
  const [isLoadingInitial, setIsLoadingInitial] = createSignal(true);
  const [hasMore, setHasMore] = createSignal(true);
  const [statusMessage, setStatusMessage] = createSignal("");
  const [statusType, setStatusType] = createSignal<"success" | "error">("success");
  const [newFeedUrl, setNewFeedUrl] = createSignal("");
  const [showFeedManager, setShowFeedManager] = createSignal(false);
  const [isAddingFeed, setIsAddingFeed] = createSignal(false);
  const [deleteConfirm, setDeleteConfirm] = createSignal<{open: boolean; feed: FeedItem | null}>({open: false, feed: null});
  
  let statusTimer: number | undefined;
  let sentinelRef: HTMLDivElement | undefined;
  let observer: IntersectionObserver | undefined;

  const feedCount = createMemo(() => feeds().length);
  const itemCount = createMemo(() => timelineItems().length);

  const setStatus = (msg: string, type: "success" | "error" = "success") => {
    setStatusMessage(msg);
    setStatusType(type);
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
    if (isLoadingMore() && !reset) return;

    if (reset) {
      setIsLoadingInitial(true);
    }
    setIsLoadingMore(true);
    
    try {
      const items = await getTimelineCompact({
        limit: 20,
        beforeId: reset ? undefined : beforeId(),
      });

      if (reset) {
        setTimelineItems(items);
        setBeforeId(items.length > 0 ? items[items.length - 1].id : null);
        setHasMore(items.length >= 20);
      } else {
        setTimelineItems((prev) => [...prev, ...items]);
        if (items.length > 0) {
          setBeforeId(items[items.length - 1].id);
        }
        setHasMore(items.length >= 20);
      }
    } catch (e) {
      console.error("Failed to fetch timeline:", e);
      setStatus("Failed to load timeline", "error");
    } finally {
      setIsLoadingMore(false);
      setIsLoadingInitial(false);
    }
  };

  const handleAddFeed = async (e?: Event) => {
    e?.preventDefault();
    const url = newFeedUrl().trim();
    if (!url) {
      setStatus("Enter a feed URL", "error");
      return;
    }

    setIsAddingFeed(true);
    try {
      await createFeed(url);
      setNewFeedUrl("");
      setStatus("Feed added!", "success");
      await loadFeeds();
      await loadTimeline(true);
    } catch (e) {
      console.error("Failed to create feed:", e);
      setStatus("Failed to add feed", "error");
    } finally {
      setIsAddingFeed(false);
    }
  };

  const handleDeleteFeed = async () => {
    const feed = deleteConfirm().feed;
    if (!feed) return;
    
    setDeleteConfirm({open: false, feed: null});
    
    try {
      await deleteFeed(feed.id);
      setStatus("Feed removed", "success");
      await loadFeeds();
      await loadTimeline(true);
    } catch (e) {
      console.error("Failed to delete feed:", e);
      setStatus("Failed to remove feed", "error");
    }
  };

  const openHttpUrl = async (raw: string) => {
    let url: URL;
    try {
      url = new URL(raw);
    } catch {
      setStatus("Invalid link", "error");
      return;
    }
    if (url.protocol !== "https:") {
      setStatus(`Blocked: ${url.protocol}`, "error");
      return;
    }
    await openUrl(url.toString());
  };

  const handleItemClick = async (link: string) => {
    try {
      await openHttpUrl(link);
    } catch (e) {
      console.error("Failed to open link:", e);
      setStatus("Failed to open link", "error");
    }
  };

  const loadMoreIfNeeded = () => {
    if (hasMore() && !isLoadingMore()) {
      loadTimeline(false);
    }
  };

  onMount(() => {
    loadFeeds();
    loadTimeline(true);
    
    // Find the scroll container (the overflow-y-auto parent from SwipeablePages)
    const findScrollParent = (el: HTMLElement | null): HTMLElement | null => {
      while (el) {
        const style = getComputedStyle(el);
        if (style.overflowY === "auto" || style.overflowY === "scroll") {
          return el;
        }
        el = el.parentElement;
      }
      return null;
    };
    
    // Use IntersectionObserver to detect when sentinel is visible
    // Need to specify root as the scroll container for proper detection
    const scrollRoot = sentinelRef ? findScrollParent(sentinelRef) : null;
    
    observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          loadMoreIfNeeded();
        }
      },
      { 
        root: scrollRoot,
        rootMargin: "300px" 
      }
    );
    
    if (sentinelRef) {
      observer.observe(sentinelRef);
    }
  });

  onCleanup(() => {
    if (statusTimer !== undefined) {
      window.clearTimeout(statusTimer);
    }
    observer?.disconnect();
  });

  return (
    <div class="space-y-4">
      {/* Section header */}
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <h2 class="text-sm font-medium text-text-secondary tracking-wide">RSS Feed</h2>
          <span class="text-xs text-text-muted">
            {itemCount()} stories from {feedCount()} sources
          </span>
          <Rss class="size-4 text-text-muted" />
        </div>
        
        {/* Manage Feeds Button */}
        <button
          onClick={() => setShowFeedManager(true)}
          class={cn(
            "size-8 rounded-xl",
            "bg-white/5 hover:bg-white/8 border border-border hover:border-accent/30",
            "flex items-center justify-center",
            "transition-all duration-200"
          )}
        >
          <Settings2 class="size-4 text-text-muted hover:text-accent" />
        </button>
      </div>

      {/* Status toast */}
      <Show when={statusMessage()}>
        <div 
          class={cn(
            "px-4 py-2 rounded-xl text-sm text-center",
            statusType() === "success" 
              ? "bg-success/10 border border-success/20 text-success"
              : "bg-error/10 border border-error/20 text-error"
          )}
        >
          {statusMessage()}
        </div>
      </Show>

      {/* Feed items */}
      <Show 
        when={!isLoadingInitial()}
        fallback={
          <div class="space-y-3">
            <For each={[1, 2, 3, 4, 5]}>
              {() => (
                <div class="rounded-2xl bg-white/3 border border-border-subtle p-4 animate-pulse">
                  <div class="h-4 w-3/4 rounded bg-white/5 mb-3" />
                  <div class="h-3 w-full rounded bg-white/5 mb-2" />
                  <div class="h-3 w-2/3 rounded bg-white/5" />
                </div>
              )}
            </For>
          </div>
        }
      >
        <Show
          when={timelineItems().length > 0}
          fallback={
            <div class="rounded-2xl border border-border-subtle bg-bg-card/50 p-8 text-center">
              <div class="inline-flex items-center justify-center size-12 rounded-full bg-white/3 mb-3">
                <Inbox class="size-5 text-text-muted" />
              </div>
              <p class="text-sm text-text-muted">No stories yet</p>
              <p class="text-xs text-text-muted/60 mt-1">Add some RSS feeds to see updates here</p>
              <button
                onClick={() => setShowFeedManager(true)}
                class={cn(
                  "mt-4 px-4 py-2 rounded-xl text-sm",
                  "bg-accent/10 hover:bg-accent/15 border border-accent/20",
                  "text-accent font-medium",
                  "transition-colors"
                )}
              >
                Add your first feed
              </button>
            </div>
          }
        >
          <div class="space-y-2">
            <For each={timelineItems()}>
              {(item) => (
                <button
                  onClick={() => handleItemClick(item.link)}
                  class={cn(
                    "group w-full text-left rounded-2xl p-4 transition-all duration-200",
                    "bg-bg-card/60 hover:bg-bg-card border border-transparent hover:border-border",
                    "focus:outline-none focus:ring-2 focus:ring-accent/20"
                  )}
                >
                  <div class="flex items-start gap-3">
                    <div class="flex-1 min-w-0">
                      {/* Title */}
                      <h3 class="text-sm font-medium text-text-primary leading-snug group-hover:text-accent transition-colors">
                        {truncate(item.title, 100)}
                      </h3>
                      
                      {/* Description */}
                      <Show when={item.description}>
                        <p class="mt-1.5 text-xs text-text-muted leading-relaxed line-clamp-2">
                          {truncate(item.description, 150)}
                        </p>
                      </Show>
                    </div>
                    
                    {/* External link indicator */}
                    <ExternalLink class="flex-shrink-0 size-3.5 text-text-muted/50 group-hover:text-accent/50 transition-colors mt-0.5" />
                  </div>
                </button>
              )}
            </For>
            
            {/* Sentinel for infinite scroll */}
            <div ref={sentinelRef} class="h-1" />
            
            {/* Loading more indicator */}
            <Show when={isLoadingMore() && !isLoadingInitial()}>
              <div class="flex items-center justify-center py-4 gap-2">
                <Loader2 class="size-4 text-accent animate-spin" />
                <span class="text-xs text-text-muted">Loading more...</span>
              </div>
            </Show>
            
            {/* End of feed */}
            <Show when={!hasMore() && timelineItems().length > 0}>
              <div class="py-4 text-center">
                <p class="text-xs text-text-muted">You've reached the end</p>
              </div>
            </Show>
          </div>
        </Show>
      </Show>

      {/* Feed Manager Modal */}
      <Dialog open={showFeedManager()} onOpenChange={setShowFeedManager}>
        <DialogPortal>
          <DialogOverlay 
            onClick={() => setShowFeedManager(false)} 
            class="bg-black/60 backdrop-blur-md"
          />
          <DialogContent 
            open={showFeedManager()} 
            onOpenChange={setShowFeedManager}
            class="fixed inset-x-4 bottom-4 top-auto rounded-3xl border-border bg-bg-secondary/95 backdrop-blur-xl p-0 max-h-[70vh] overflow-hidden"
          >
            {/* Modal Header */}
            <div class="sticky top-0 z-10 bg-bg-secondary/95 backdrop-blur-xl border-b border-border px-5 py-4">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <div class="size-10 rounded-xl bg-accent/10 flex items-center justify-center">
                    <Rss class="size-5 text-accent" />
                  </div>
                  <div>
                    <DialogTitle class="text-lg font-semibold">Manage Feeds</DialogTitle>
                    <DialogDescription class="text-xs text-text-muted">
                      {feedCount()} active sources
                    </DialogDescription>
                  </div>
                </div>
                <button
                  onClick={() => setShowFeedManager(false)}
                  class="size-9 rounded-xl bg-white/5 hover:bg-white/10 flex items-center justify-center transition-colors"
                >
                  <X class="size-4 text-text-secondary" />
                </button>
              </div>
              
              {/* Add Feed Form */}
              <form onSubmit={handleAddFeed} class="mt-4 flex gap-2">
                <input
                  type="url"
                  value={newFeedUrl()}
                  onInput={(e) => setNewFeedUrl(e.currentTarget.value)}
                  placeholder="https://example.com/feed.xml"
                  class={cn(
                    "flex-1 rounded-xl border border-border bg-white/5 px-4 py-2.5",
                    "text-sm text-text-primary placeholder-text-muted",
                    "focus:border-accent focus:outline-none focus:ring-2 focus:ring-accent/20",
                    "transition-all duration-200"
                  )}
                />
                <button
                  type="submit"
                  disabled={isAddingFeed()}
                  class={cn(
                    "px-4 rounded-xl",
                    "bg-accent hover:bg-accent-hover",
                    "text-white font-medium text-sm",
                    "flex items-center justify-center gap-2",
                    "transition-all duration-200",
                    "disabled:opacity-50 disabled:cursor-not-allowed"
                  )}
                >
                  <Show when={isAddingFeed()} fallback={<Plus class="size-4" />}>
                    <Loader2 class="size-4 animate-spin" />
                  </Show>
                </button>
              </form>
            </div>
            
            {/* Feed List */}
            <div class="overflow-y-auto px-5 py-4 max-h-[45vh]">
              <Show
                when={feeds().length > 0}
                fallback={
                  <div class="py-8 text-center">
                    <div class="size-16 rounded-full bg-white/5 mx-auto mb-4 flex items-center justify-center">
                      <Rss class="size-7 text-text-muted" />
                    </div>
                    <p class="text-sm text-text-muted">No feeds yet</p>
                    <p class="text-xs text-text-muted/60 mt-1">Add a URL above to get started</p>
                  </div>
                }
              >
                <div class="space-y-2">
                  <For each={feeds()}>
                    {(feed) => (
                      <div class="group flex items-center gap-3 p-3 rounded-2xl bg-white/3 hover:bg-white/5 border border-transparent hover:border-border transition-all">
                        <div class="flex-1 min-w-0">
                          <p class="text-sm font-medium text-text-primary truncate">
                            {feed.title ?? "Untitled Feed"}
                          </p>
                          <p class="text-[11px] text-text-muted truncate">
                            {feed.url}
                          </p>
                        </div>
                        <button
                          onClick={() => setDeleteConfirm({open: true, feed})}
                          class={cn(
                            "size-8 rounded-xl flex items-center justify-center flex-shrink-0",
                            "bg-transparent hover:bg-error/10 border border-transparent hover:border-error/20",
                            "text-text-muted hover:text-error",
                            "opacity-0 group-hover:opacity-100",
                            "transition-all duration-200"
                          )}
                        >
                          <Trash2 class="size-4" />
                        </button>
                      </div>
                    )}
                  </For>
                </div>
              </Show>
            </div>
          </DialogContent>
        </DialogPortal>
      </Dialog>

      {/* Delete Confirmation Dialog */}
      <ConfirmDialog
        open={deleteConfirm().open}
        onOpenChange={(open) => setDeleteConfirm({open, feed: deleteConfirm().feed})}
        title="Remove Feed"
        description={`Remove "${deleteConfirm().feed?.title ?? 'this feed'}" from your sources?`}
        confirmLabel="Remove"
        cancelLabel="Keep"
        onConfirm={handleDeleteFeed}
      />
    </div>
  );
};

export default RssFeed;
