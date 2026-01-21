import type { Component, Accessor } from "solid-js";
import { Home, Rss, Boxes } from "lucide-solid";
import { cn } from "@/lib/utils";

export type NavPage = "home" | "rss" | "docker";

interface BottomNavProps {
  currentPage: Accessor<NavPage>;
  onNavigate: (page: NavPage) => void;
}

const BottomNav: Component<BottomNavProps> = (props) => {
  return (
    <nav class="fixed bottom-0 left-0 right-0 z-50">
      {/* Gradient fade */}
      <div class="absolute inset-x-0 -top-6 h-6 bg-gradient-to-t from-bg-primary to-transparent pointer-events-none" />
      
      <div class="bg-bg-primary/80 backdrop-blur-xl border-t border-border-subtle">
        <div class="mx-auto flex h-16 max-w-md items-center justify-around px-6">
          <button
            onClick={() => props.onNavigate("home")}
            class={cn(
              "relative flex flex-col items-center justify-center gap-1 px-4 py-2 rounded-xl transition-all duration-200",
              props.currentPage() === "home"
                ? "text-accent"
                : "text-text-muted hover:text-text-secondary"
            )}
          >
            <Home class="size-5" />
            <span class="text-[10px] font-medium tracking-wide">Home</span>
            {props.currentPage() === "home" && (
              <span class="absolute -bottom-0.5 left-1/2 -translate-x-1/2 w-1 h-1 rounded-full bg-accent" />
            )}
          </button>

          <button
            onClick={() => props.onNavigate("rss")}
            class={cn(
              "relative flex flex-col items-center justify-center gap-1 px-4 py-2 rounded-xl transition-all duration-200",
              props.currentPage() === "rss"
                ? "text-accent"
                : "text-text-muted hover:text-text-secondary"
            )}
          >
            <Rss class="size-5" />
            <span class="text-[10px] font-medium tracking-wide">RSS</span>
            {props.currentPage() === "rss" && (
              <span class="absolute -bottom-0.5 left-1/2 -translate-x-1/2 w-1 h-1 rounded-full bg-accent" />
            )}
          </button>

          <button
            onClick={() => props.onNavigate("docker")}
            class={cn(
              "relative flex flex-col items-center justify-center gap-1 px-4 py-2 rounded-xl transition-all duration-200",
              props.currentPage() === "docker"
                ? "text-accent"
                : "text-text-muted hover:text-text-secondary"
            )}
          >
            <Boxes class="size-5" />
            <span class="text-[10px] font-medium tracking-wide">Docker</span>
            {props.currentPage() === "docker" && (
              <span class="absolute -bottom-0.5 left-1/2 -translate-x-1/2 w-1 h-1 rounded-full bg-accent" />
            )}
          </button>
        </div>
      </div>
    </nav>
  );
};

export default BottomNav;
