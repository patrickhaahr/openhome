import { createResource, createSignal, Show, Suspense, Switch, Match, For } from "solid-js";
import {
  getAdguardStatus,
  enableProtection,
  disableProtection,
  pauseProtection,
} from "@/api/adguard";
import { Shield, ShieldOff, Pause, Play, RefreshCw, Clock } from "lucide-solid";
import { cn } from "@/lib/utils";

interface AdguardControlProps {
  class?: string;
}

const formatTimeUntil = (isoString: string | null, durationMs: number): string => {
  // Try using the ISO string first
  if (isoString) {
    const now = new Date();
    const until = new Date(isoString);
    const diffMs = until.getTime() - now.getTime();
    
    if (diffMs <= 0) return "Expired";
    
    const minutes = Math.floor(diffMs / 60000);
    const hours = Math.floor(minutes / 60);
    
    if (hours > 0) return `${hours}h ${minutes % 60}m`;
    return `${minutes}m`;
  }
  
  // Fallback to duration if no ISO string
  if (durationMs > 0) {
    const minutes = Math.floor(durationMs / 60000);
    const hours = Math.floor(minutes / 60);
    
    if (hours > 0) return `${hours}h ${minutes % 60}m`;
    if (minutes > 0) return `${minutes}m`;
    return "<1m";
  }
  
  return "";
};

const PAUSE_OPTIONS = [5, 15, 30, 60];

const AdguardControl = (props: AdguardControlProps) => {
  const [status, { refetch }] = createResource(getAdguardStatus);
  const [isLoading, setIsLoading] = createSignal<string | null>(null);
  const [showPauseOptions, setShowPauseOptions] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  
  const handleEnable = async () => {
    setIsLoading("enable");
    setError(null);
    try {
      await enableProtection();
      refetch();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed");
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
      setError(err instanceof Error ? err.message : "Failed");
    } finally {
      setIsLoading(null);
    }
  };
  
  const handlePause = async (minutes: number) => {
    setIsLoading("pause");
    setError(null);
    setShowPauseOptions(false);
    try {
      await pauseProtection(minutes);
      refetch();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed");
    } finally {
      setIsLoading(null);
    }
  };
  
  return (
    <div class={cn("relative", props.class)}>
      {/* Main control card */}
      <div class="rounded-3xl bg-bg-card border border-border p-6">
        <Suspense
          fallback={
            <div class="flex flex-col items-center gap-6 py-4">
              <div class="size-20 rounded-full bg-white/5 animate-pulse" />
              <div class="h-4 w-24 rounded bg-white/5 animate-pulse" />
            </div>
          }
        >
          <Show
            when={!status.error}
            fallback={
              <div class="text-center py-8">
                <ShieldOff class="mx-auto size-12 text-error/50 mb-3" />
                <p class="text-text-secondary text-sm">Connection failed</p>
                <button
                  onClick={() => refetch()}
                  class="mt-3 text-xs text-accent hover:text-accent-hover transition-colors"
                >
                  Retry
                </button>
              </div>
            }
          >
            <Show when={status()}>
              {(s) => {
                // Compute status states from the unwrapped value
                // Check both protection_disabled_until AND protection_disabled_duration
                const enabled = () => s().protection_enabled;
                const paused = () => !s().protection_enabled && (!!s().protection_disabled_until || s().protection_disabled_duration > 0);
                const disabled = () => !s().protection_enabled && !s().protection_disabled_until && s().protection_disabled_duration === 0;
                
                return (
                <div class="flex flex-col items-center">
                  {/* Central status indicator */}
                  <div class="relative mb-6">
                    {/* Glow ring */}
                    <div
                      class={cn(
                        "absolute inset-0 rounded-full blur-xl transition-all duration-500",
                        enabled() && "bg-success/20",
                        paused() && "bg-warning/20",
                        disabled() && "bg-error/20"
                      )}
                    />
                    
                    {/* Shield icon container */}
                    <div
                      class={cn(
                        "relative size-24 rounded-full flex items-center justify-center transition-all duration-300",
                        "border-2",
                        enabled() && "border-success/30 bg-success/5",
                        paused() && "border-warning/30 bg-warning/5",
                        disabled() && "border-error/30 bg-error/5"
                      )}
                    >
                      <Switch>
                        <Match when={enabled()}>
                          <Shield class="size-10 text-success" />
                        </Match>
                        <Match when={paused()}>
                          <Clock class="size-10 text-warning" />
                        </Match>
                        <Match when={disabled()}>
                          <ShieldOff class="size-10 text-error" />
                        </Match>
                      </Switch>
                    </div>
                  </div>
                  
                  {/* Status text */}
                  <div class="text-center mb-6">
                    <p class={cn(
                      "text-sm font-medium tracking-wide",
                      enabled() && "text-success",
                      paused() && "text-warning",
                      disabled() && "text-error"
                    )}>
                      {enabled() && "Protected"}
                      {paused() && `Paused · ${formatTimeUntil(s().protection_disabled_until, s().protection_disabled_duration)}`}
                      {disabled() && "Unprotected"}
                    </p>
                    <p class="text-text-muted text-xs mt-1">
                      AdGuard {s().running ? `v${s().version}` : "offline"}
                    </p>
                  </div>
                  
                  {/* Error toast */}
                  <Show when={error()}>
                    <div class="w-full mb-4 px-4 py-2 rounded-xl bg-error/10 border border-error/20 text-error text-xs text-center">
                      {error()}
                    </div>
                  </Show>
                  
                  {/* Action buttons */}
                  <div class="flex items-center justify-center gap-3">
                    <Switch>
                      {/* When protection is OFF or PAUSED - show enable button */}
                      <Match when={!enabled()}>
                        <button
                          onClick={handleEnable}
                          disabled={isLoading() === "enable"}
                          class={cn(
                            "flex items-center justify-center gap-2 px-6 py-3 rounded-2xl",
                            "bg-success/10 hover:bg-success/15 border border-success/20",
                            "text-success text-sm font-medium",
                            "transition-all duration-200",
                            "disabled:opacity-50 disabled:cursor-not-allowed"
                          )}
                        >
                          <Play class={cn(
                            "size-4",
                            isLoading() === "enable" && "animate-pulse"
                          )} />
                          <span>Enable</span>
                        </button>
                      </Match>
                      
                      {/* When protection is ON - show pause and disable buttons */}
                      <Match when={enabled()}>
                        {/* Pause button */}
                        <div class="relative">
                          <button
                            onClick={() => setShowPauseOptions(!showPauseOptions())}
                            disabled={isLoading() === "pause"}
                            class={cn(
                              "flex items-center justify-center size-12 rounded-2xl",
                              "bg-warning/10 hover:bg-warning/15 border border-warning/20",
                              "text-warning transition-all duration-200",
                              "disabled:opacity-50 disabled:cursor-not-allowed"
                            )}
                            title="Pause protection"
                          >
                            <Pause class={cn(
                              "size-5",
                              isLoading() === "pause" && "animate-pulse"
                            )} />
                          </button>
                          
                          {/* Pause duration popup */}
                          <Show when={showPauseOptions()}>
                            <div class="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 z-10">
                              <div class="flex gap-1 p-1.5 rounded-2xl bg-bg-elevated border border-border shadow-xl">
                                <For each={PAUSE_OPTIONS}>
                                  {(mins) => (
                                    <button
                                      onClick={() => handlePause(mins)}
                                      class={cn(
                                        "px-3 py-1.5 rounded-xl text-xs font-medium",
                                        "bg-transparent hover:bg-white/5",
                                        "text-text-secondary hover:text-text-primary",
                                        "transition-colors"
                                      )}
                                    >
                                      {mins >= 60 ? `${mins / 60}h` : `${mins}m`}
                                    </button>
                                  )}
                                </For>
                              </div>
                              {/* Arrow */}
                              <div class="absolute top-full left-1/2 -translate-x-1/2 -mt-px">
                                <div class="border-8 border-transparent border-t-border" />
                              </div>
                            </div>
                          </Show>
                        </div>
                        
                        {/* Disable button */}
                        <button
                          onClick={handleDisable}
                          disabled={isLoading() === "disable"}
                          class={cn(
                            "flex items-center justify-center size-12 rounded-2xl",
                            "bg-error/10 hover:bg-error/15 border border-error/20",
                            "text-error transition-all duration-200",
                            "disabled:opacity-50 disabled:cursor-not-allowed"
                          )}
                          title="Disable protection"
                        >
                          <ShieldOff class={cn(
                            "size-5",
                            isLoading() === "disable" && "animate-pulse"
                          )} />
                        </button>
                      </Match>
                    </Switch>
                    
                    {/* Refresh button - hidden on mobile, visible on desktop */}
                    <button
                      onClick={() => refetch()}
                      disabled={status.loading}
                      class={cn(
                        "items-center justify-center size-12 rounded-2xl",
                        "bg-white/5 hover:bg-white/8 border border-border",
                        "text-text-muted hover:text-text-secondary",
                        "transition-all duration-200",
                        "disabled:opacity-50 disabled:cursor-not-allowed",
                        "hidden md:flex" // Hide on mobile, show on desktop
                      )}
                      title="Refresh status"
                    >
                      <RefreshCw class={cn(
                        "size-4",
                        status.loading && "animate-spin"
                      )} />
                    </button>
                  </div>
                </div>
              )}}
            </Show>
          </Show>
        </Suspense>
      </div>
      
      {/* Click outside to close pause options */}
      <Show when={showPauseOptions()}>
        <div
          class="fixed inset-0 z-0"
          onClick={() => setShowPauseOptions(false)}
        />
      </Show>
    </div>
  );
};

export default AdguardControl;
