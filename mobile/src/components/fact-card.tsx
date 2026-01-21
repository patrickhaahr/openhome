import { Component, createResource, Show, Suspense } from "solid-js";
import { getRandomFact } from "../api/facts";
import { RefreshCw } from "lucide-solid";
import { cn } from "@/lib/utils";

const FactCard: Component = () => {
  const [fact, { refetch }] = createResource(getRandomFact);

  return (
    <div class="relative">
      {/* Refresh button - hidden on mobile (use pull-to-refresh), visible on desktop */}
      <button
        onClick={() => refetch()}
        disabled={fact.loading}
        class={cn(
          "absolute -top-1 right-0 p-2 rounded-full",
          "text-text-muted hover:text-text-secondary hover:bg-white/5",
          "transition-all duration-200",
          "disabled:opacity-50 disabled:cursor-not-allowed",
          "hidden md:flex" // Hide on mobile, show on desktop
        )}
        title="New fact"
      >
        <RefreshCw 
          class={cn(
            "size-4 transition-transform duration-500",
            fact.loading && "animate-spin"
          )} 
        />
      </button>

      {/* The fact */}
      <Suspense
        fallback={
          <div class="space-y-3">
            <div class="h-5 w-full rounded bg-white/5 animate-pulse" />
            <div class="h-5 w-4/5 rounded bg-white/5 animate-pulse" />
            <div class="h-5 w-3/5 rounded bg-white/5 animate-pulse" />
          </div>
        }
      >
        <Show
          when={!fact.error}
          fallback={
            <p class="text-error/80 text-sm italic">
              Could not load fact
            </p>
          }
        >
          <p class="text-lg leading-relaxed text-text-primary/90 font-light tracking-tight md:pr-10">
            {fact()?.text}
          </p>
        </Show>
      </Suspense>
    </div>
  );
};

export default FactCard;
