import { Component, createResource, Show, Switch, Match } from "solid-js";
import { listDockerContainers, type DockerContainerStatus } from "@/api/docker";
import { cn } from "@/lib/utils";

type HealthStatus = "healthy" | "unhealthy" | "sleeping" | "loading" | "error";

// Docker whale icon as SVG
const DockerIcon = (props: { class?: string }) => (
  <svg 
    viewBox="0 0 24 24" 
    fill="currentColor" 
    class={props.class}
  >
    <path d="M13.983 11.078h2.119a.186.186 0 0 0 .186-.185V9.006a.186.186 0 0 0-.186-.186h-2.119a.185.185 0 0 0-.185.185v1.888c0 .102.083.185.185.185m-2.954-5.43h2.118a.186.186 0 0 0 .186-.186V3.574a.186.186 0 0 0-.186-.185h-2.118a.185.185 0 0 0-.185.185v1.888c0 .102.082.185.185.186m0 2.716h2.118a.187.187 0 0 0 .186-.186V6.29a.186.186 0 0 0-.186-.185h-2.118a.185.185 0 0 0-.185.185v1.887c0 .102.082.185.185.186m-2.93 0h2.12a.186.186 0 0 0 .184-.186V6.29a.185.185 0 0 0-.185-.185H8.1a.185.185 0 0 0-.185.185v1.887c0 .102.083.185.185.186m-2.964 0h2.119a.186.186 0 0 0 .185-.186V6.29a.185.185 0 0 0-.185-.185H5.136a.186.186 0 0 0-.186.185v1.887c0 .102.084.185.186.186m5.893 2.715h2.118a.186.186 0 0 0 .186-.185V9.006a.186.186 0 0 0-.186-.186h-2.118a.185.185 0 0 0-.185.185v1.888c0 .102.082.185.185.185m-2.93 0h2.12a.185.185 0 0 0 .184-.185V9.006a.185.185 0 0 0-.184-.186h-2.12a.185.185 0 0 0-.184.185v1.888c0 .102.083.185.185.185m-2.964 0h2.119a.185.185 0 0 0 .185-.185V9.006a.185.185 0 0 0-.185-.186H5.136a.186.186 0 0 0-.186.186v1.887c0 .102.084.185.186.185m-2.92 0h2.12a.185.185 0 0 0 .184-.185V9.006a.185.185 0 0 0-.184-.186h-2.12a.185.185 0 0 0-.184.185v1.888c0 .102.082.185.185.185M23.763 9.89c-.065-.051-.672-.51-1.954-.51-.338.001-.676.03-1.01.087-.248-1.7-1.653-2.53-1.716-2.566l-.344-.199-.226.327c-.284.438-.49.922-.612 1.43-.23.97-.09 1.882.403 2.661-.595.332-1.55.413-1.744.42H.751a.751.751 0 0 0-.75.748 11.376 11.376 0 0 0 .692 4.062c.545 1.428 1.355 2.48 2.41 3.124 1.18.723 3.1 1.137 5.275 1.137.983.003 1.963-.086 2.93-.266a12.248 12.248 0 0 0 3.823-1.389c.98-.567 1.86-1.288 2.61-2.136 1.252-1.418 1.998-2.997 2.553-4.4h.221c1.372 0 2.215-.549 2.68-1.009.309-.293.55-.65.707-1.046l.098-.288Z"/>
  </svg>
);

const getOverallHealth = (containers: DockerContainerStatus[]): HealthStatus => {
  if (containers.length === 0) return "sleeping";
  
  const runningContainers = containers.filter(c => c.state.toLowerCase() === "running");
  
  if (runningContainers.length === 0) return "sleeping";
  
  // Check if any running container is unhealthy
  const hasUnhealthy = runningContainers.some(c => {
    const status = c.status.toLowerCase();
    const health = c.health_status?.toLowerCase();
    return status.includes("unhealthy") || (health && health !== "healthy");
  });
  
  return hasUnhealthy ? "unhealthy" : "healthy";
};

const DockerHealth: Component = () => {
  const [containersData] = createResource(listDockerContainers);
  
  const health = (): HealthStatus => {
    if (containersData.loading) return "loading";
    if (containersData.error) return "error";
    if (!containersData()) return "sleeping";
    return getOverallHealth(containersData()!.containers);
  };
  
  const runningCount = () => {
    const containers = containersData()?.containers ?? [];
    return containers.filter(c => c.state.toLowerCase() === "running").length;
  };
  
  const totalCount = () => containersData()?.containers.length ?? 0;

  return (
    <div class="rounded-2xl bg-bg-card border border-border p-4">
      <div class="flex items-center gap-3">
        {/* Docker icon */}
        <DockerIcon
          class={cn(
            "size-8 transition-colors duration-300",
            health() === "healthy" && "text-success",
            health() === "unhealthy" && "text-warning",
            health() === "sleeping" && "text-text-muted",
            health() === "loading" && "text-accent animate-pulse",
            health() === "error" && "text-error"
          )}
        />
        
        {/* Status text */}
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2">
            <span class="text-sm font-medium text-text-primary">
              <Switch>
                <Match when={health() === "loading"}>
                  Docker
                </Match>
                <Match when={health() === "healthy"}>
                  Docker Healthy
                </Match>
                <Match when={health() === "unhealthy"}>
                  Docker Unhealthy
                </Match>
                <Match when={health() === "sleeping"}>
                  Docker Idle
                </Match>
                <Match when={health() === "error"}>
                  Docker Offline
                </Match>
              </Switch>
            </span>
          </div>
          
          <Show when={health() !== "loading" && health() !== "error"}>
            <p class="text-xs text-text-muted mt-0.5">
              <Switch>
                <Match when={health() === "sleeping"}>
                  No containers running
                </Match>
                <Match when={health() === "healthy" || health() === "unhealthy"}>
                  {runningCount()} of {totalCount()} running
                </Match>
              </Switch>
            </p>
          </Show>
          
          <Show when={health() === "error"}>
            <p class="text-xs text-text-muted mt-0.5">
              Could not connect
            </p>
          </Show>
        </div>
        
        {/* Mini status indicator */}
        <div
          class={cn(
            "size-2 rounded-full transition-colors duration-300",
            health() === "healthy" && "bg-success",
            health() === "unhealthy" && "bg-warning",
            health() === "sleeping" && "bg-text-muted/50",
            health() === "loading" && "bg-accent animate-pulse",
            health() === "error" && "bg-error"
          )}
        />
      </div>
    </div>
  );
};

export default DockerHealth;
