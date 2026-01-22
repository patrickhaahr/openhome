import type { Component, Accessor } from "solid-js";
import { Home, Boxes } from "lucide-solid";
import { cn } from "@/lib/utils";

export type NavPage = "home" | "docker";

interface BottomNavProps {
  currentPage: Accessor<NavPage>;
  onNavigate: (page: NavPage) => void;
  visible?: Accessor<boolean>;
}

const BottomNav: Component<BottomNavProps> = (props) => {
  const isVisible = () => props.visible?.() ?? true;
  
  return (
    <nav 
      class="fixed bottom-0 left-0 right-0 z-50 flex justify-center pb-6 transition-transform duration-300 ease-out"
      style={{ transform: isVisible() ? "translateY(0)" : "translateY(calc(100% + 1.5rem))" }}
    >
      <div class="flex items-center gap-1 rounded-full bg-bg-secondary/90 backdrop-blur-xl p-1.5 shadow-lg shadow-black/20 border border-white/5">
        <button
          onClick={() => props.onNavigate("home")}
          class={cn(
            "flex items-center justify-center p-2.5 rounded-full transition-all duration-200",
            props.currentPage() === "home"
              ? "bg-accent text-white"
              : "text-text-muted"
          )}
        >
          <Home class="size-5" />
        </button>

        <button
          onClick={() => props.onNavigate("docker")}
          class={cn(
            "flex items-center justify-center p-2.5 rounded-full transition-all duration-200",
            props.currentPage() === "docker"
              ? "bg-accent text-white"
              : "text-text-muted"
          )}
        >
          <Boxes class="size-5" />
        </button>
      </div>
    </nav>
  );
};

export default BottomNav;
