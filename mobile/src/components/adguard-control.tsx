import { createResource, createSignal, Show, Suspense, Switch, Match } from "solid-js";
import {
  getAdguardStatus,
  enableProtection,
  disableProtection,
  pauseProtection,
} from "@/api/adguard";
import { Button } from "./ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "./ui/card";
import { Skeleton } from "./ui/skeleton";
import { cn } from "@/lib/utils";

interface AdguardControlProps {
  class?: string;
}

const formatTimeUntil = (isoString: string | null): string => {
  if (!isoString) return "";
  
  const now = new Date();
  const until = new Date(isoString);
  const diffMs = until.getTime() - now.getTime();
  
  if (diffMs <= 0) return "Already expired";
  
  const minutes = Math.floor(diffMs / 60000);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  
  if (days > 0) return `${days}d ${hours % 24}h`;
  if (hours > 0) return `${hours}h ${minutes % 60}m`;
  return `${minutes}m`;
};

const StatusBadge = (props: { enabled: boolean; disabledUntil: string | null }) => {
  const isEnabled = () => props.enabled;
  const isPaused = () => !props.enabled && !!props.disabledUntil;
  const isDisabled = () => !props.enabled && !props.disabledUntil;
  
  return (
    <div class="flex items-center gap-2">
      <div
        class={cn(
          "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold tracking-wide uppercase",
          isEnabled() && "bg-emerald-500/15 text-emerald-400 border border-emerald-500/30",
          isPaused() && "bg-amber-500/15 text-amber-400 border border-amber-500/30",
          isDisabled() && "bg-red-500/15 text-red-400 border border-red-500/30"
        )}
      >
        <span
          class={cn(
            "w-1.5 h-1.5 rounded-full",
            isEnabled() && "bg-emerald-400 animate-pulse",
            isPaused() && "bg-amber-400",
            isDisabled() && "bg-red-400"
          )}
        />
        {isEnabled() && "Protection ON"}
        {isPaused() && "Paused"}
        {isDisabled() && "Protection OFF"}
      </div>
      {isPaused() && props.disabledUntil && (
        <span class="text-xs text-muted-foreground">
          until {formatTimeUntil(props.disabledUntil)}
        </span>
      )}
    </div>
  );
};

const AdguardControl = (props: AdguardControlProps) => {
  const [status, { refetch }] = createResource(getAdguardStatus);
  const [isLoading, setIsLoading] = createSignal<string | null>(null);
  const [pauseMinutes, setPauseMinutes] = createSignal(5);
  const [error, setError] = createSignal<string | null>(null);
  
  const handleEnable = async () => {
    setIsLoading("enable");
    setError(null);
    try {
      await enableProtection();
      refetch();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to enable protection");
    } finally {
      setIsLoading(null);
    }
  };
  
  const handleDisable = async () => {
    setIsLoading("disable");
    setError(null);
    try {
      await disableProtection();
      refetch();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to disable protection");
    } finally {
      setIsLoading(null);
    }
  };
  
  const handlePause = async () => {
    setIsLoading("pause");
    setError(null);
    try {
      await pauseProtection(pauseMinutes());
      refetch();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to pause protection");
    } finally {
      setIsLoading(null);
    }
  };
  
  return (
    <Card class={cn("w-full", props.class)}>
      <CardHeader>
        <div class="flex items-center justify-between">
          <div>
            <CardTitle class="flex items-center gap-2">
              <span>AdGuard Protection</span>
            </CardTitle>
            <CardDescription>Network-level ad blocking control</CardDescription>
          </div>
          <Switch>
            <Match when={status.loading}>
              <div class="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            </Match>
            <Match when={status()}>
              <StatusBadge 
                enabled={status()!.protection_enabled} 
                disabledUntil={status()!.protection_disabled_until}
              />
            </Match>
          </Switch>
        </div>
      </CardHeader>
      
      <CardContent>
        <Suspense
          fallback={
            <div class="space-y-3">
              <Skeleton class="h-4 w-full" />
              <Skeleton class="h-4 w-[70%]" />
              <div class="flex gap-2 pt-2">
                <Skeleton class="h-9 w-20" />
                <Skeleton class="h-9 w-24" />
              </div>
            </div>
          }
        >
          <Show
            when={!status.error}
            fallback={
              <div class="rounded-lg bg-destructive/10 p-4 text-sm text-destructive">
                <p class="font-medium">Failed to load status</p>
                <p class="mt-1 opacity-80">{status.error?.message}</p>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => refetch()}
                  class="mt-3"
                >
                  Try Again
                </Button>
              </div>
            }
          >
            <Show when={status()}>
              {(s) => (
                <div class="space-y-4">
                  {/* Version and running info */}
                  <div class="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm text-muted-foreground">
                    <span>v{s().version}</span>
                    <span class="flex items-center gap-1.5">
                      <span
                        class={cn(
                          "w-1.5 h-1.5 rounded-full",
                          s().running ? "bg-emerald-500" : "bg-red-500"
                        )}
                      />
                      {s().running ? "Running" : "Not running"}
                    </span>
                  </div>
                  
                  {/* Error message */}
                  <Show when={error()}>
                    {(e) => (
                      <div class="rounded-lg bg-destructive/10 p-3 text-sm text-destructive animate-in fade-in slide-in-from-top-1">
                        {e()}
                      </div>
                    )}
                  </Show>
                  
                  {/* Pause duration input */}
                  <Show when={s().protection_enabled}>
                    <div class="flex flex-wrap items-center gap-3">
                      <label class="text-sm font-medium text-foreground">
                        Pause for
                      </label>
                      <div class="flex items-center gap-2">
                        <input
                          type="number"
                          min="1"
                          max="1440"
                          value={pauseMinutes()}
                          onInput={(e) => setPauseMinutes(Math.max(1, parseInt(e.currentTarget.value) || 5))}
                          disabled={isLoading() === "pause"}
                          class="w-20 h-9 rounded-md border border-border bg-background px-3 py-1 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:opacity-50"
                        />
                        <span class="text-sm text-muted-foreground">min</span>
                      </div>
                    </div>
                  </Show>
                </div>
              )}
            </Show>
          </Show>
        </Suspense>
      </CardContent>
      
      <CardFooter class="flex flex-wrap gap-2">
        <Show when={status()}>
          {(s) => (
            <Switch>
              <Match when={!s().protection_enabled}>
                <Button
                  onClick={handleEnable}
                  disabled={isLoading() === "enable"}
                  class="flex-1 min-w-[120px]"
                >
                  <Show when={isLoading() === "enable"} fallback={s().protection_disabled_until ? "Resume Protection" : "Enable Protection"}>
                    {s().protection_disabled_until ? "Resuming..." : "Enabling..."}
                  </Show>
                </Button>
              </Match>

              <Match when={s().protection_enabled}>
                <div class="flex flex-wrap gap-2 w-full">
                  <Button
                    variant="outline"
                    onClick={handlePause}
                    disabled={isLoading() === "pause"}
                    class="flex-1 min-w-[100px]"
                  >
                    <Show when={isLoading() === "pause"} fallback={`Pause ${pauseMinutes()} min`}>
                      Pausing...
                    </Show>
                  </Button>
                  <Button
                    variant="destructive"
                    onClick={handleDisable}
                    disabled={isLoading() === "disable"}
                    class="flex-1 min-w-[100px]"
                  >
                    <Show when={isLoading() === "disable"} fallback="Disable Protection">
                      Disabling...
                    </Show>
                  </Button>
                </div>
              </Match>
            </Switch>
          )}
        </Show>
        
        {/* Refresh button when loaded */}
        <Show when={!status.loading && status()}>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => refetch()}
            class="size-9"
            title="Refresh status"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              class={cn("transition-transform", status.loading && "animate-spin")}
            >
              <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
              <path d="M3 3v5h5" />
              <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
              <path d="M16 16h5v5" />
            </svg>
          </Button>
        </Show>
      </CardFooter>
    </Card>
  );
};

export default AdguardControl;
