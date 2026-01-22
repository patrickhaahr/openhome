import type { Component } from "solid-js";
import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
  For,
  Match,
  on,
  onCleanup,
  onMount,
  Show,
  Switch,
} from "solid-js";
import { 
  Terminal, 
  RotateCw, 
  Play, 
  Square, 
  ChevronLeft,
  Clock,
  Network,
  ArrowDownToLine,
} from "lucide-solid";

import {
  listDockerContainers,
  getDockerLogs,
  restartDockerContainer,
  startDockerContainer,
  stopDockerContainer,
  type DockerContainerStatus,
} from "@/api/docker";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { formatUptime, formatPorts } from "@/utils/formatters";

const RESTART_TIMEOUT_SECONDS = 10;
const STOP_TIMEOUT_SECONDS = 10;
const TAIL_PRESETS = [50, 200, 500, 1000];

type FilterKey = "all" | "healthy" | "unhealthy" | "stopped";
type HealthStatus = "healthy" | "unhealthy" | "stopped";

// Docker whale SVG component
const DockerWhale = (props: { class?: string; health: HealthStatus }) => (
  <svg 
    viewBox="0 0 24 24" 
    fill="currentColor" 
    class={props.class}
  >
    <path d="M13.983 11.078h2.119a.186.186 0 0 0 .186-.185V9.006a.186.186 0 0 0-.186-.186h-2.119a.185.185 0 0 0-.185.185v1.888c0 .102.083.185.185.185m-2.954-5.43h2.118a.186.186 0 0 0 .186-.186V3.574a.186.186 0 0 0-.186-.185h-2.118a.185.185 0 0 0-.185.185v1.888c0 .102.082.185.185.186m0 2.716h2.118a.187.187 0 0 0 .186-.186V6.29a.186.186 0 0 0-.186-.185h-2.118a.185.185 0 0 0-.185.185v1.887c0 .102.082.185.185.186m-2.93 0h2.12a.186.186 0 0 0 .184-.186V6.29a.185.185 0 0 0-.185-.185H8.1a.185.185 0 0 0-.185.185v1.887c0 .102.083.185.185.186m-2.964 0h2.119a.186.186 0 0 0 .185-.186V6.29a.185.185 0 0 0-.185-.185H5.136a.186.186 0 0 0-.186.185v1.887c0 .102.084.185.186.186m5.893 2.715h2.118a.186.186 0 0 0 .186-.185V9.006a.186.186 0 0 0-.186-.186h-2.118a.185.185 0 0 0-.185.185v1.888c0 .102.082.185.185.185m-2.93 0h2.12a.185.185 0 0 0 .184-.185V9.006a.185.185 0 0 0-.184-.186h-2.12a.185.185 0 0 0-.184.185v1.888c0 .102.083.185.185.185m-2.964 0h2.119a.185.185 0 0 0 .185-.185V9.006a.185.185 0 0 0-.185-.186H5.136a.186.186 0 0 0-.186.186v1.887c0 .102.084.185.186.185m-2.92 0h2.12a.185.185 0 0 0 .184-.185V9.006a.185.185 0 0 0-.184-.186h-2.12a.185.185 0 0 0-.184.185v1.888c0 .102.082.185.185.185M23.763 9.89c-.065-.051-.672-.51-1.954-.51-.338.001-.676.03-1.01.087-.248-1.7-1.653-2.53-1.716-2.566l-.344-.199-.226.327c-.284.438-.49.922-.612 1.43-.23.97-.09 1.882.403 2.661-.595.332-1.55.413-1.744.42H.751a.751.751 0 0 0-.75.748 11.376 11.376 0 0 0 .692 4.062c.545 1.428 1.355 2.48 2.41 3.124 1.18.723 3.1 1.137 5.275 1.137.983.003 1.963-.086 2.93-.266a12.248 12.248 0 0 0 3.823-1.389c.98-.567 1.86-1.288 2.61-2.136 1.252-1.418 1.998-2.997 2.553-4.4h.221c1.372 0 2.215-.549 2.68-1.009.309-.293.55-.65.707-1.046l.098-.288Z"/>
  </svg>
);

const statusMeta = (container: DockerContainerStatus): { label: string; health: HealthStatus } => {
  const state = container.state.toLowerCase();
  const status = container.status.toLowerCase();
  const health = container.health_status?.toLowerCase();
  const hasUnhealthyStatus = status.includes("unhealthy") || (health ? health !== "healthy" : false);

  if (state === "running" && !hasUnhealthyStatus) {
    return { label: health === "healthy" ? "Healthy" : "Running", health: "healthy" };
  }

  if (state === "running" && hasUnhealthyStatus) {
    return { label: "Unhealthy", health: "unhealthy" };
  }

  return { label: "Stopped", health: "stopped" };
};

const getOverallHealth = (containers: DockerContainerStatus[]): HealthStatus => {
  if (containers.length === 0) return "stopped";
  
  const runningContainers = containers.filter(c => c.state.toLowerCase() === "running");
  if (runningContainers.length === 0) return "stopped";
  
  const hasUnhealthy = runningContainers.some(c => {
    const status = c.status.toLowerCase();
    const health = c.health_status?.toLowerCase();
    return status.includes("unhealthy") || (health && health !== "healthy");
  });
  
  return hasUnhealthy ? "unhealthy" : "healthy";
};

const Docker: Component = () => {
  const [filter, setFilter] = createSignal<FilterKey>("all");
  const [selectedContainer, setSelectedContainer] = createSignal<DockerContainerStatus | null>(null);
  const [logsOpen, setLogsOpen] = createSignal(false);
  const [logsLoading, setLogsLoading] = createSignal(false);
  const [logsError, setLogsError] = createSignal<string | null>(null);
  const [logsText, setLogsText] = createSignal("");
  const [tailCount, setTailCount] = createSignal(200);
  const [lastLogsSignature, setLastLogsSignature] = createSignal<string | null>(null);
  const [autoScroll, setAutoScroll] = createSignal(true);
  const [actionError, setActionError] = createSignal<string | null>(null);
  const [restartingContainer, setRestartingContainer] = createSignal<string | null>(null);

  let logsViewport: HTMLDivElement | undefined;
  let refreshTimer: number | undefined;

  const [containers, { refetch }] = createResource(listDockerContainers);

  const filteredContainers = createMemo(() => {
    const items = containers()?.containers ?? [];
    if (filter() === "all") return items;
    return items.filter((container) => statusMeta(container).health === filter());
  });

  const overallHealth = createMemo((): HealthStatus => {
    if (containers.loading || containers.error) return "stopped";
    return getOverallHealth(containers()?.containers ?? []);
  });

  const stats = createMemo(() => {
    const items = containers()?.containers ?? [];
    const running = items.filter(c => c.state.toLowerCase() === "running").length;
    const total = items.length;
    const healthy = items.filter(c => statusMeta(c).health === "healthy").length;
    const unhealthy = items.filter(c => statusMeta(c).health === "unhealthy").length;
    const stopped = items.filter(c => statusMeta(c).health === "stopped").length;
    return { running, total, healthy, unhealthy, stopped };
  });

  const fetchLogs = async () => {
    const container = selectedContainer();
    if (!container) return;

    setLogsLoading(true);
    setLogsError(null);
    setActionError(null);
    try {
      const logs = await getDockerLogs(container.name, {
        tail: tailCount(),
        timestamps: true,
      });
      setLogsText(logs);
      setLastLogsSignature(`${container.name}-${tailCount()}-${logs.length}`);
    } catch (err) {
      setLogsError(err instanceof Error ? err.message : "Failed to fetch logs");
    } finally {
      setLogsLoading(false);
    }
  };

  const handleOpenLogs = (container: DockerContainerStatus) => {
    setSelectedContainer(container);
    setLogsOpen(true);
    setLogsText("");
    setLogsError(null);
    setLastLogsSignature(null);
  };

  const handleLogsBack = () => {
    setLogsOpen(false);
    setLogsError(null);
    setLogsText("");
    setSelectedContainer(null);
    setLastLogsSignature(null);
  };

  createEffect(on([logsOpen], () => {
    if (logsOpen()) fetchLogs();
  }));

  createEffect(on([tailCount], () => {
    if (logsOpen()) fetchLogs();
  }, { defer: true }));

  createEffect(() => {
    if (logsOpen() && logsViewport && lastLogsSignature()) {
      if (autoScroll()) {
        logsViewport.scrollTop = logsViewport.scrollHeight;
      }
    }
  });

  onMount(() => {
    refreshTimer = window.setInterval(() => refetch(), 20000);
  });

  onCleanup(() => {
    if (refreshTimer !== undefined) window.clearInterval(refreshTimer);
  });

  const healthColor = (health: HealthStatus) => {
    switch (health) {
      case "healthy": return "text-success";
      case "unhealthy": return "text-warning";
      case "stopped": return "text-text-muted";
    }
  };

  const healthBg = (health: HealthStatus) => {
    switch (health) {
      case "healthy": return "bg-success/10 border-success/20";
      case "unhealthy": return "bg-warning/10 border-warning/20";
      case "stopped": return "bg-text-muted/10 border-text-muted/20";
    }
  };

  return (
    <div class="min-h-[calc(100vh-5rem)]">
      <Show
        when={!logsOpen()}
        fallback={
          /* ============ LOGS VIEW ============ */
          <div class="flex h-full flex-col animate-in fade-in slide-in-from-right-4 duration-300">
            {/* Logs Header */}
            <header class="mb-6">
              <button
                onClick={handleLogsBack}
                class="flex items-center gap-1 text-text-muted hover:text-text-primary transition-colors mb-4"
              >
                <ChevronLeft class="size-5" />
                <span class="text-sm">Back</span>
              </button>
              
              <div class="flex items-center gap-3">
                <div class={cn(
                  "size-10 rounded-xl flex items-center justify-center border",
                  healthBg(statusMeta(selectedContainer()!).health)
                )}>
                  <Terminal class={cn("size-5", healthColor(statusMeta(selectedContainer()!).health))} />
                </div>
                <div>
                  <h1 class="text-lg font-semibold text-text-primary">
                    {selectedContainer()?.name}
                  </h1>
                  <p class="text-xs text-text-muted">
                    {tailCount()} lines
                  </p>
                </div>
              </div>
            </header>

            {/* Logs Controls */}
            <div class="flex items-center gap-2 mb-4 overflow-x-auto pb-2 scrollbar-none">
              <For each={TAIL_PRESETS}>
                {(preset) => (
                  <button
                    onClick={() => setTailCount(preset)}
                    class={cn(
                      "px-3 py-1.5 rounded-full text-xs font-medium transition-all whitespace-nowrap",
                      tailCount() === preset
                        ? "bg-text-primary text-bg-primary"
                        : "bg-bg-tertiary text-text-secondary hover:bg-bg-elevated"
                    )}
                  >
                    {preset}
                  </button>
                )}
              </For>
              <div class="w-px h-4 bg-border mx-1" />
              <button
                onClick={() => setAutoScroll(!autoScroll())}
                class={cn(
                  "p-2 rounded-full transition-all",
                  autoScroll()
                    ? "bg-text-primary text-bg-primary"
                    : "bg-bg-tertiary text-text-secondary hover:bg-bg-elevated"
                )}
                title={autoScroll() ? "Auto-scroll on" : "Auto-scroll off"}
              >
                <ArrowDownToLine class="size-4" />
              </button>
            </div>

            {/* Logs Content */}
            <div class="flex-1 rounded-2xl border border-border bg-bg-secondary/60 backdrop-blur-sm overflow-hidden">
              <ScrollArea
                class="h-[calc(100vh-16rem)]"
                viewportRef={(el) => { logsViewport = el; }}
              >
                <div class="p-4">
                  <Switch>
                    <Match when={logsLoading()}>
                      <div class="space-y-2">
                        <Skeleton class="h-4 w-full" />
                        <Skeleton class="h-4 w-[90%]" />
                        <Skeleton class="h-4 w-[80%]" />
                        <Skeleton class="h-4 w-[95%]" />
                      </div>
                    </Match>
                    <Match when={logsError()}>
                      <div class="rounded-xl bg-error/10 border border-error/20 px-4 py-3 text-error text-sm">
                        {logsError()}
                      </div>
                    </Match>
                    <Match when={logsText()}>
                      <pre class="whitespace-pre-wrap text-[11px] leading-relaxed text-text-secondary font-mono">
                        {logsText()}
                      </pre>
                    </Match>
                    <Match when={!logsText() && !logsLoading()}>
                      <p class="text-sm text-text-muted text-center py-8">No logs returned</p>
                    </Match>
                  </Switch>
                </div>
              </ScrollArea>
            </div>
          </div>
        }
      >
        {/* ============ MAIN VIEW ============ */}
        <div class="space-y-8 animate-in fade-in duration-500">
          
          {/* Hero Section - Docker Whale */}
          <section class="flex flex-col items-center pt-4 pb-8">
            {/* Docker Whale */}
            <div>
              <DockerWhale 
                health={overallHealth()}
                class={cn(
                  "size-28 transition-colors duration-700",
                  healthColor(overallHealth()),
                  containers.loading && "animate-pulse"
                )}
              />
            </div>

            {/* Stats below whale */}
            <div class="mt-8 flex items-center justify-center text-center">
              <div class="w-20 flex flex-col items-center">
                <p class="text-2xl font-semibold text-text-primary tabular-nums">
                  {stats().running}
                </p>
                <p class="text-xs text-text-muted mt-0.5">Running</p>
              </div>
              <div class="w-px h-8 bg-border mx-4" />
              <div class="w-20 flex flex-col items-center">
                <p class="text-2xl font-semibold text-text-primary tabular-nums">
                  {stats().total}
                </p>
                <p class="text-xs text-text-muted mt-0.5">Total</p>
              </div>
            </div>
          </section>

          {/* Error display */}
          <Show when={actionError()}>
            <div class="rounded-xl bg-error/10 border border-error/20 px-4 py-3 text-error text-sm animate-in fade-in slide-in-from-top-2">
              {actionError()}
            </div>
          </Show>

          {/* Filter Pills */}
          <section class="flex items-center justify-center gap-1.5 overflow-x-auto pb-2 scrollbar-none">
            <For each={[
              { key: "all" as FilterKey, label: "All", count: stats().total },
              { key: "healthy" as FilterKey, label: "Healthy", count: stats().healthy },
              { key: "unhealthy" as FilterKey, label: "Unhealthy", count: stats().unhealthy },
              { key: "stopped" as FilterKey, label: "Stopped", count: stats().stopped },
            ]}>
              {(item) => (
                <button
                  onClick={() => setFilter(item.key)}
                  class={cn(
                    "flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium transition-all whitespace-nowrap",
                    filter() === item.key
                      ? "bg-text-primary text-bg-primary"
                      : "bg-bg-tertiary/60 text-text-secondary hover:bg-bg-elevated border border-border"
                  )}
                >
                  <span>{item.label}</span>
                  <span class={cn(
                    "text-[10px] tabular-nums",
                    filter() === item.key ? "text-bg-primary/70" : "text-text-muted"
                  )}>
                    {item.count}
                  </span>
                </button>
              )}
            </For>
          </section>

          {/* Container List */}
          <section>
            <Switch>
              <Match when={containers.loading}>
                <div class="space-y-3">
                  <Skeleton class="h-28 w-full rounded-2xl" />
                  <Skeleton class="h-28 w-full rounded-2xl" />
                  <Skeleton class="h-28 w-full rounded-2xl" />
                </div>
              </Match>
              <Match when={containers.error}>
                <div class="rounded-2xl bg-error/5 border border-error/10 px-5 py-8 text-center">
                  <p class="text-error text-sm">
                    {containers.error?.message ?? "Failed to load containers"}
                  </p>
                </div>
              </Match>
              <Match when={filteredContainers().length === 0}>
                <div class="rounded-2xl bg-bg-secondary/40 border border-border px-5 py-12 text-center">
                  <DockerWhale health="stopped" class="size-12 text-text-muted/30 mx-auto mb-3" />
                  <p class="text-sm text-text-muted">No containers match this filter</p>
                </div>
              </Match>
              <Match when={filteredContainers().length > 0}>
                <div class="space-y-3">
                  <For each={filteredContainers()}>
                    {(container, index) => {
                      const meta = statusMeta(container);
                      return (
                        <div 
                          class="group rounded-2xl bg-bg-secondary/60 border border-border backdrop-blur-sm overflow-hidden transition-all hover:border-border/80 animate-in fade-in slide-in-from-bottom-2"
                          style={{ "animation-delay": `${index() * 50}ms` }}
                        >
                          {/* Main row */}
                          <div class="p-4 flex items-center gap-4">
                            {/* Status indicator */}
                            <div class={cn(
                              "size-10 rounded-xl flex items-center justify-center border shrink-0 transition-all",
                              healthBg(meta.health)
                            )}>
                              <DockerWhale 
                                health={meta.health}
                                class={cn("size-5 transition-colors", healthColor(meta.health))}
                              />
                            </div>

                            {/* Container info */}
                            <div class="flex-1 min-w-0">
                              <div class="flex items-center gap-2">
                                <h3 class="text-sm font-medium text-text-primary truncate">
                                  {container.name}
                                </h3>
                                <Show when={container.restart_count > 0}>
                                  <span class="text-[10px] text-text-muted bg-bg-tertiary px-1.5 py-0.5 rounded-full tabular-nums">
                                    {container.restart_count}x
                                  </span>
                                </Show>
                              </div>
                              <p class="text-xs text-text-muted truncate mt-0.5">
                                {container.image}
                              </p>
                            </div>

                            {/* Action buttons - icons only */}
                            <div class="flex items-center gap-1.5 shrink-0">
                              <button
                                onClick={() => handleOpenLogs(container)}
                                class="p-2.5 rounded-xl bg-bg-tertiary/60 text-text-secondary hover:text-text-primary hover:bg-bg-tertiary transition-all"
                                title="View logs"
                              >
                                <Terminal class="size-4" />
                              </button>

                              <Show when={container.state !== "running"}>
                                <button
                                  onClick={async () => {
                                    setActionError(null);
                                    try {
                                      await startDockerContainer(container.name);
                                      refetch();
                                    } catch (err) {
                                      setActionError(err instanceof Error ? err.message : "Failed to start");
                                    }
                                  }}
                                  class="p-2.5 rounded-xl bg-success/10 text-success hover:bg-success/20 transition-all"
                                  title="Start container"
                                >
                                  <Play class="size-4" />
                                </button>
                              </Show>

                              <Show when={container.state === "running"}>
                                <button
                                  onClick={async () => {
                                    setActionError(null);
                                    try {
                                      await stopDockerContainer(container.name, STOP_TIMEOUT_SECONDS);
                                      refetch();
                                    } catch (err) {
                                      setActionError(err instanceof Error ? err.message : "Failed to stop");
                                    }
                                  }}
                                  class="p-2.5 rounded-xl bg-error/10 text-error hover:bg-error/20 transition-all"
                                  title="Stop container"
                                >
                                  <Square class="size-4" />
                                </button>
                              </Show>

                              <button
                                onClick={async () => {
                                  setActionError(null);
                                  setRestartingContainer(container.name);
                                  try {
                                    await restartDockerContainer(container.name, RESTART_TIMEOUT_SECONDS);
                                    refetch();
                                  } catch (err) {
                                    setActionError(err instanceof Error ? err.message : "Failed to restart");
                                  } finally {
                                    setRestartingContainer(null);
                                  }
                                }}
                                disabled={restartingContainer() === container.name}
                                class={cn(
                                  "p-2.5 rounded-xl bg-bg-tertiary/60 text-text-secondary hover:text-text-primary hover:bg-bg-tertiary transition-all",
                                  restartingContainer() === container.name && "opacity-50"
                                )}
                                title="Restart container"
                              >
                                <RotateCw class={cn(
                                  "size-4",
                                  restartingContainer() === container.name && "animate-spin"
                                )} />
                              </button>
                            </div>
                          </div>

                          {/* Expandable details */}
                          <div class="px-4 pb-4 pt-0">
                            <div class="flex items-center gap-4 text-xs text-text-muted">
                              <div class="flex items-center gap-1.5">
                                <Clock class="size-3.5" />
                                <span>{formatUptime(container.uptime_seconds)}</span>
                              </div>
                              <div class="flex items-center gap-1.5 truncate">
                                <Network class="size-3.5 shrink-0" />
                                <span class="truncate">{formatPorts(container.ports)}</span>
                              </div>
                            </div>
                          </div>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </Match>
            </Switch>
          </section>
        </div>
      </Show>
    </div>
  );
};

export default Docker;
