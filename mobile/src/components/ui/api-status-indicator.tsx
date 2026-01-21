import { Component, createResource, Show } from "solid-js";
import { getHealthStatus } from "../../api/health";
import { baseUrl } from "../../stores/config";
import { cn } from "@/lib/utils";

const fetchHealthStatus = async (): Promise<boolean> => {
  const url = baseUrl();
  if (!url) return false;

  try {
    const response = await getHealthStatus();
    return response.status === "ok";
  } catch {
    return false;
  }
};

const ApiStatusIndicator: Component = () => {
  const [isOnline] = createResource(baseUrl, fetchHealthStatus);

  return (
    <div class="group relative">
      {/* The indicator dot */}
      <div
        class={cn(
          "relative size-2.5 rounded-full transition-all duration-300",
          isOnline.loading && "bg-text-muted animate-pulse",
          !isOnline.loading && isOnline() && "bg-success",
          !isOnline.loading && !isOnline() && "bg-error"
        )}
      >
        {/* Glow effect */}
        <Show when={!isOnline.loading}>
          <div
            class={cn(
              "absolute inset-0 rounded-full blur-sm opacity-60",
              isOnline() ? "bg-success" : "bg-error"
            )}
          />
        </Show>
      </div>
      
      {/* Tooltip on hover */}
      <div class="pointer-events-none absolute right-0 top-full mt-2 opacity-0 transition-opacity duration-200 group-hover:opacity-100">
        <div class="whitespace-nowrap rounded-lg bg-bg-elevated px-3 py-1.5 text-xs text-text-secondary shadow-lg border border-border">
          <Show
            when={!isOnline.loading}
            fallback={<span>Checking...</span>}
          >
            <span>API {isOnline() ? "Online" : "Offline"}</span>
          </Show>
        </div>
      </div>
    </div>
  );
};

export default ApiStatusIndicator;
