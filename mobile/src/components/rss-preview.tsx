import { Component, createResource, For, Show } from "solid-js";
import { getTimelineCompact, type TimelineCompactItem } from "@/api/rss";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Newspaper, ExternalLink, Inbox } from "lucide-solid";
import { cn } from "@/lib/utils";

const fetchLatestItems = async (): Promise<TimelineCompactItem[]> => {
  const items = await getTimelineCompact({ limit: 3 });
  return items;
};

// Truncate text elegantly
const truncate = (text: string | null | undefined, maxLength: number): string => {
  if (!text) return "";
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength).trim() + "...";
};

// Strip HTML tags from description
const stripHtml = (html: string | null | undefined): string => {
  if (!html) return "";
  return html.replace(/<[^>]*>/g, "").trim();
};

const RssPreview: Component = () => {
  const [items] = createResource(fetchLatestItems);

  const handleItemClick = async (link: string) => {
    try {
      const url = new URL(link);
      if (url.protocol === "https:") {
        await openUrl(url.toString());
      }
    } catch (e) {
      console.error("Failed to open link:", e);
    }
  };

  return (
    <div class="space-y-4">
      {/* Section header */}
      <div class="flex items-center gap-2">
        <Newspaper class="size-4 text-text-muted" />
        <h2 class="text-sm font-medium text-text-secondary tracking-wide">Latest Reads</h2>
      </div>

      {/* Feed items */}
      <Show
        when={!items.loading}
        fallback={
          <div class="space-y-3">
            <div class="h-20 rounded-2xl bg-white/3 animate-pulse" />
            <div class="h-20 rounded-2xl bg-white/3 animate-pulse" />
            <div class="h-20 rounded-2xl bg-white/3 animate-pulse" />
          </div>
        }
      >
        <Show
          when={!items.error && items()?.length}
          fallback={
            <div class="rounded-2xl border border-border-subtle bg-bg-card/50 p-8 text-center">
              <div class="inline-flex items-center justify-center size-12 rounded-full bg-white/3 mb-3">
                <Inbox class="size-5 text-text-muted" />
              </div>
              <p class="text-sm text-text-muted">No stories yet</p>
              <p class="text-xs text-text-muted/60 mt-1">Add some RSS feeds to see updates here</p>
            </div>
          }
        >
          <div class="space-y-2">
            <For each={items()}>
              {(item, index) => (
                <button
                  onClick={() => handleItemClick(item.link)}
                  class={cn(
                    "group w-full text-left rounded-2xl p-4 transition-all duration-200",
                    "bg-bg-card/60 hover:bg-bg-card border border-transparent hover:border-border",
                    "focus:outline-none focus:ring-2 focus:ring-accent/20"
                  )}
                >
                  <div class="flex items-start gap-3">
                    {/* Number indicator - subtle design element */}
                    <div class="flex-shrink-0 size-6 rounded-lg bg-accent/5 flex items-center justify-center">
                      <span class="text-xs font-medium text-accent/70">{index() + 1}</span>
                    </div>
                    
                    <div class="flex-1 min-w-0">
                      {/* Title */}
                      <h3 class="text-sm font-medium text-text-primary leading-snug group-hover:text-accent transition-colors">
                        {truncate(item.title, 80)}
                      </h3>
                      
                      {/* Description */}
                      <Show when={item.description}>
                        <p class="mt-1.5 text-xs text-text-muted leading-relaxed line-clamp-2">
                          {truncate(stripHtml(item.description), 120)}
                        </p>
                      </Show>
                    </div>
                    
                    {/* External link indicator */}
                    <ExternalLink class="flex-shrink-0 size-3.5 text-text-muted/50 group-hover:text-accent/50 transition-colors mt-0.5" />
                  </div>
                </button>
              )}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
};

export default RssPreview;
