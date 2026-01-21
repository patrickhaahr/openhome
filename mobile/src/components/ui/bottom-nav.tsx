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
      <nav class="fixed bottom-0 left-0 right-0 z-50 border-t border-border bg-bg-secondary/95 backdrop-blur-sm">
         <div class="mx-auto flex h-16 max-w-lg items-center justify-around gap-1 px-2">
          <button
            onClick={() => props.onNavigate("home")}
            class={cn(
              "flex flex-col items-center justify-center gap-1 px-6 py-2 transition-colors",
              props.currentPage() === "home"
                ? "text-accent"
                : "text-text-secondary hover:text-text-primary"
            )}
        >
          <Home class="size-6" />
          <span class="text-xs font-medium">Home</span>
        </button>

          <button
            onClick={() => props.onNavigate("rss")}
            class={cn(
              "flex flex-col items-center justify-center gap-1 px-4 py-2 transition-colors",
              props.currentPage() === "rss"
                ? "text-accent"
                : "text-text-secondary hover:text-text-primary"
            )}
        >
          <Rss class="size-6" />
          <span class="text-xs font-medium">RSS</span>
        </button>

          <button
            onClick={() => props.onNavigate("docker")}
            class={cn(
              "flex flex-col items-center justify-center gap-1 px-4 py-2 transition-colors",
              props.currentPage() === "docker"
                ? "text-accent"
                : "text-text-secondary hover:text-text-primary"
            )}
        >
          <Boxes class="size-6" />
          <span class="text-xs font-medium">Docker</span>
        </button>
      </div>
    </nav>
  );
};

export default BottomNav;
