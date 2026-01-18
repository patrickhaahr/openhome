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
import { RefreshCcw, Terminal, RotateCw, Filter } from "lucide-solid";

import {
  listDockerContainers,
  getDockerLogs,
  restartDockerContainer,
  type DockerContainerStatus,
} from "@/api/docker";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import { ConfirmDialog, useDialogState } from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

const RESTART_TIMEOUT_SECONDS = 10;
const TAIL_PRESETS = [50, 200, 500, 1000];

const formatUptime = (seconds: number | null): string => {
  if (seconds === null) return "Unknown uptime";
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ${minutes % 60}m`;
  const days = Math.floor(hours / 24);
  return `${days}d ${hours % 24}h`;
};

const formatPorts = (ports: string[]): string => {
  if (ports.length === 0) return "No ports exposed";
  return ports.join(", ");
};

const toTitleCase = (value: string): string =>
  value.replace(/(^\w|_\w)/g, (match) => match.replace("_", " ").toUpperCase());

const statusMeta = (container: DockerContainerStatus) => {
  const state = container.state.toLowerCase();
  const status = container.status.toLowerCase();
  const health = container.health_status?.toLowerCase();
  const hasUnhealthyStatus = status.includes("unhealthy") || (health ? health !== "healthy" : false);

  if (state === "running" && !hasUnhealthyStatus) {
    return {
      label: health === "healthy" ? "Healthy" : "Running",
      class: "bg-[#16a34a]/15 text-[#22c55e] border-[#16a34a]/40",
      dot: "bg-[#22c55e]",
      filter: "healthy",
    };
  }

  if (state === "running" && hasUnhealthyStatus) {
    return {
      label: health ? toTitleCase(health) : "Unhealthy",
      class: "bg-[#f59e0b]/20 text-[#fcd34d] border-[#f59e0b]/40",
      dot: "bg-[#fcd34d]",
      filter: "unhealthy",
    };
  }

  return {
    label: "Stopped",
    class: "bg-[#64748b]/20 text-[#cbd5f5] border-[#94a3b8]/40",
    dot: "bg-[#cbd5f5]",
    filter: "stopped",
  };
};

const Docker: Component = () => {
  const [filter, setFilter] = createSignal("all");
  const filters = [
    { key: "all", label: "All" },
    { key: "healthy", label: "Healthy" },
    { key: "unhealthy", label: "Unhealthy" },
    { key: "stopped", label: "Stopped" },
  ];
  const [selectedContainer, setSelectedContainer] = createSignal<DockerContainerStatus | null>(null);
  const [logsOpen, setLogsOpen] = createSignal(false);
  const [logsLoading, setLogsLoading] = createSignal(false);
  const [logsError, setLogsError] = createSignal<string | null>(null);
  const [logsText, setLogsText] = createSignal("");
  const [tailCount, setTailCount] = createSignal(200);
  const [lastLogsSignature, setLastLogsSignature] = createSignal<string | null>(null);
  const [autoScroll, setAutoScroll] = createSignal(true);
  const [restartTarget, setRestartTarget] = createSignal<DockerContainerStatus | null>(null);
  const [actionError, setActionError] = createSignal<string | null>(null);
  const restartDialog = useDialogState(false);

  let logsViewport: HTMLDivElement | undefined;
  let refreshTimer: number | undefined;

  const [containers, { refetch }] = createResource(listDockerContainers);

  const filteredContainers = createMemo(() => {
    const items = containers()?.containers ?? [];
    if (filter() === "all") return items;

    return items.filter((container) => statusMeta(container).filter === filter());
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

  const handleRestart = (container: DockerContainerStatus) => {
    setRestartTarget(container);
    setActionError(null);
    restartDialog.setOpen(true);
  };

  const confirmRestart = async () => {
    const target = restartTarget();
    if (!target) return;

    setActionError(null);
    try {
      await restartDockerContainer(target.name, RESTART_TIMEOUT_SECONDS);
      restartDialog.setOpen(false);
      setRestartTarget(null);
      refetch();
    } catch (err) {
      console.error("Failed to restart container:", err);
      setActionError(err instanceof Error ? err.message : "Failed to restart container");
    }
  };

  const handleLogsBack = () => {
    setLogsOpen(false);
    setLogsError(null);
    setLogsText("");
    setSelectedContainer(null);
    setLastLogsSignature(null);
  };

  createEffect(on([logsOpen], () => {
    if (logsOpen()) {
      fetchLogs();
    }
  }));

  createEffect(on([tailCount], () => {
    if (logsOpen()) {
      fetchLogs();
    }
  }, { defer: true }));

  createEffect(() => {
    if (logsOpen() && logsViewport && lastLogsSignature()) {
      if (autoScroll()) {
        logsViewport.scrollTop = logsViewport.scrollHeight;
      }
    }
  });

  onMount(() => {
    refreshTimer = window.setInterval(() => {
      refetch();
    }, 20000);
  });

  onCleanup(() => {
    if (refreshTimer !== undefined) {
      window.clearInterval(refreshTimer);
    }
  });

  return (
    <div class="min-h-[calc(100vh-5rem)]">
      <ConfirmDialog
        open={restartDialog.open()}
        onOpenChange={restartDialog.setOpen}
        title="Restart container"
        description={`Restart ${restartTarget()?.name ?? "this container"}? (timeout ${RESTART_TIMEOUT_SECONDS}s)`}
        confirmLabel="Restart"
        onConfirm={confirmRestart}
        isConfirmDisabled={!restartTarget()}
      />

      <Show
        when={!logsOpen()}
        fallback={
          <div class="flex h-full flex-col">
            <header class="mb-4 flex items-center justify-between gap-3">
              <div>
                <h1 class="text-xl font-semibold text-text-primary sm:text-2xl">
                  {selectedContainer()?.name ?? "Logs"}
                </h1>
                <p class="text-sm text-text-secondary">
                  Tail of {tailCount()} lines, auto-scroll {autoScroll() ? "on" : "off"}
                </p>
              </div>
              <Button variant="ghost" size="sm" onClick={handleLogsBack}>
                Back
              </Button>
            </header>

            <div class="flex flex-wrap items-center gap-2 pb-4">
              <label class="text-xs uppercase tracking-wide text-text-muted">Tail</label>
              <For each={TAIL_PRESETS}>
                {(preset) => (
                  <Button
                    variant={tailCount() === preset ? "secondary" : "outline"}
                    size="sm"
                    onClick={() => setTailCount(preset)}
                  >
                    {preset}
                  </Button>
                )}
              </For>
              <Button
                variant={autoScroll() ? "secondary" : "outline"}
                size="sm"
                onClick={() => setAutoScroll(!autoScroll())}
              >
                {autoScroll() ? "Auto-scroll on" : "Auto-scroll off"}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => fetchLogs()}
                disabled={logsLoading()}
                class="ml-auto"
              >
                <RefreshCcw class={cn("size-4", logsLoading() && "animate-spin")} />
                Refresh
              </Button>
            </div>

            <div class="flex-1 rounded-2xl border border-border bg-bg-secondary/80">
              <ScrollArea
                class="h-[calc(100vh-15rem)]"
                viewportRef={(el) => {
                  logsViewport = el;
                }}
              >
                <div class="p-4">
                  <Switch>
                    <Match when={logsLoading()}>
                      <div class="space-y-2">
                        <Skeleton class="h-4 w-full" />
                        <Skeleton class="h-4 w-[90%]" />
                        <Skeleton class="h-4 w-[80%]" />
                      </div>
                    </Match>
                    <Match when={logsError()}>
                      <div class="rounded-lg bg-error/10 px-4 py-3 text-error">
                        {logsError()}
                      </div>
                    </Match>
                    <Match when={logsText()}>
                      <pre class="whitespace-pre-wrap text-xs leading-relaxed text-text-primary">
                        {logsText()}
                      </pre>
                    </Match>
                    <Match when={!logsText() && !logsLoading()}>
                      <p class="text-sm text-text-muted">No logs returned.</p>
                    </Match>
                  </Switch>
                </div>
              </ScrollArea>
            </div>
          </div>
        }
      >
        <div class="space-y-6">
          <header class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <h1 class="text-xl font-semibold text-text-primary sm:text-2xl">
                Docker
              </h1>
              <p class="text-sm text-text-secondary">
                Container status, logs, and quick restarts.
              </p>
            </div>
            <div class="flex items-center gap-2">
              <Button variant="outline" size="sm" onClick={() => refetch()}>
                <RefreshCcw class={cn("size-4", containers.loading && "animate-spin")} />
                Refresh
              </Button>
            </div>
          </header>

          <Show when={actionError()}>
            <div class="rounded-lg bg-error/10 px-4 py-3 text-error">
              {actionError()}
            </div>
          </Show>

          <Card class="border-border/60 bg-bg-secondary/60">
            <CardHeader class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <div>
                <CardTitle class="flex items-center gap-2">
                  <Filter class="size-4" />
                  Status filters
                </CardTitle>
                <CardDescription>Filter containers by runtime state.</CardDescription>
              </div>
              <div class="flex flex-wrap gap-2">
                <For each={filters}>
                  {(item) => (
                    <Button
                      variant={filter() === item.key ? "secondary" : "outline"}
                      size="sm"
                      onClick={() => setFilter(item.key)}
                    >
                      {item.label}
                    </Button>
                  )}
                </For>
              </div>
            </CardHeader>
          </Card>

          <Switch>
            <Match when={containers.loading}>
              <div class="space-y-4">
                <Skeleton class="h-24 w-full" />
                <Skeleton class="h-24 w-full" />
                <Skeleton class="h-24 w-full" />
              </div>
            </Match>
            <Match when={containers.error}>
              <div class="rounded-lg bg-error/10 px-4 py-3 text-error">
                {containers.error?.message ?? "Failed to load Docker containers"}
              </div>
            </Match>
            <Match when={filteredContainers().length === 0}>
              <Card>
                <CardContent class="flex flex-col items-center gap-2 p-6 text-center">
                  <p class="text-sm text-text-muted">
                    No containers match the current filter.
                  </p>
                </CardContent>
              </Card>
            </Match>
            <Match when={filteredContainers().length > 0}>
              <div class="space-y-4">
                <For each={filteredContainers()}>
                  {(container) => {
                    const meta = statusMeta(container);
                    return (
                      <Card class="border-border/60 bg-bg-secondary/70">
                        <CardHeader class="flex flex-col gap-3 pb-3 sm:flex-row sm:items-start sm:justify-between">
                          <div class="space-y-2">
                            <div class="flex items-center gap-2">
                              <Badge class={cn("border", meta.class)}>
                                <span class={cn("mr-1 inline-block size-1.5 rounded-full", meta.dot)} />
                                {meta.label}
                              </Badge>
                              <span class="text-xs text-text-muted">Restarted {container.restart_count}x</span>
                            </div>
                            <div>
                              <CardTitle class="text-lg">{container.name}</CardTitle>
                              <CardDescription class="text-sm text-text-secondary">
                                {container.image}
                              </CardDescription>
                            </div>
                          </div>
                          <div class="flex flex-wrap gap-2">
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => handleOpenLogs(container)}
                            >
                              <Terminal class="size-4" />
                              Logs
                            </Button>
                            <Button
                              variant="destructive"
                              size="sm"
                              onClick={() => handleRestart(container)}
                            >
                              <RotateCw class="size-4" />
                              Restart
                            </Button>
                          </div>
                        </CardHeader>
                        <CardContent class="pt-0">
                          <div class="grid gap-3 rounded-xl bg-bg-tertiary/60 p-4 text-sm text-text-secondary">
                            <div class="flex flex-wrap items-center justify-between gap-3">
                              <span class="text-xs uppercase tracking-wide text-text-muted">Uptime</span>
                              <span class="font-medium text-text-primary">{formatUptime(container.uptime_seconds)}</span>
                            </div>
                            <div class="flex flex-wrap items-center justify-between gap-3">
                              <span class="text-xs uppercase tracking-wide text-text-muted">Ports</span>
                              <span class="text-right text-sm text-text-primary">{formatPorts(container.ports)}</span>
                            </div>
                          </div>
                        </CardContent>
                      </Card>
                    );
                  }}
                </For>
              </div>
            </Match>
          </Switch>
        </div>
      </Show>
    </div>
  );
};

export default Docker;
