import type { Component } from "solid-js";
import { createSignal, Show, onMount } from "solid-js";
import { Lock, Fingerprint, Loader2, AlertCircle } from "lucide-solid";
import { auth } from "@/stores/auth";
import { cn } from "@/lib/utils";

const ApiKeyUnlock: Component = () => {
  const [isLoading, setIsLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [hasAttempted, setHasAttempted] = createSignal(false);

  const handleUnlock = async () => {
    if (isLoading()) return;

    setIsLoading(true);
    setError(null);
    setHasAttempted(true);

    try {
      await auth.unlock();
    } catch (e: any) {
      const errorMessage = e?.toString?.() || String(e);
      if (errorMessage.includes("cancelled")) {
        setError("Authentication cancelled. Try again.");
      } else {
        setError("Authentication failed. Please try again.");
      }
      console.error("Unlock failed:", e);
    } finally {
      setIsLoading(false);
    }
  };

  onMount(() => {
    const timer = setTimeout(() => {
      handleUnlock();
    }, 300);

    return () => clearTimeout(timer);
  });

  return (
    <div class="relative flex min-h-screen flex-col items-center justify-center bg-bg-primary px-6 overflow-hidden">
      {/* Animated gradient orbs */}
      <div class="pointer-events-none absolute -top-40 -right-40 h-80 w-80 rounded-full bg-accent/8 blur-3xl animate-pulse" />
      <div class="pointer-events-none absolute top-1/3 -left-40 h-64 w-64 rounded-full bg-accent/5 blur-3xl" />
      <div class="pointer-events-none absolute -bottom-32 right-1/4 h-48 w-48 rounded-full bg-accent/6 blur-3xl animate-pulse" style={{ "animation-delay": "1s" }} />

      {/* Main content */}
      <div class="relative w-full max-w-sm animate-in fade-in slide-in-from-bottom-4 duration-700">
        {/* Lock icon hero */}
        <div class="flex justify-center mb-12">
          <div class={cn(
            "relative size-24 rounded-3xl flex items-center justify-center transition-all duration-500",
            error()
              ? "bg-error/10 border-2 border-error/30"
              : "bg-bg-secondary/80 border border-border backdrop-blur-xl"
          )}>
            {/* Glow effect */}
            <div class={cn(
              "absolute inset-0 rounded-3xl blur-xl transition-all duration-500",
              error()
                ? "bg-error/20"
                : isLoading()
                  ? "bg-accent/20 animate-pulse"
                  : "bg-accent/10"
            )} />

            {/* Icon */}
            <div class="relative">
              <Show when={!isLoading()}>
                <Lock class={cn(
                  "size-10 transition-colors duration-300",
                  error() ? "text-error" : "text-accent"
                )} />
              </Show>
              <Show when={isLoading()}>
                <Loader2 class="size-10 text-accent animate-spin" />
              </Show>
            </div>
          </div>
        </div>

        {/* Title and subtitle */}
        <div class="text-center mb-8 space-y-2">
          <h1 class="text-2xl font-semibold text-text-primary">
            Unlock API Key
          </h1>
          <p class="text-sm text-text-muted">
            Authenticate to access your API key
          </p>
        </div>

        {/* Biometric button */}
        <button
          onClick={handleUnlock}
          disabled={isLoading()}
          class={cn(
            "w-full relative rounded-2xl border bg-bg-secondary/60 backdrop-blur-sm px-6 py-4 flex items-center justify-center gap-3 transition-all duration-300",
            error()
              ? "border-error/40 hover:border-error/50"
              : "border-accent/30 hover:border-accent/50 hover:bg-accent/5",
            isLoading() && "cursor-wait opacity-70"
          )}
        >
          <Show when={isLoading()}>
            <Loader2 class="size-5 text-accent animate-spin" />
          </Show>
          <Show when={!isLoading()}>
            <Fingerprint class="size-5 text-accent" />
          </Show>
          <span class={cn(
            "text-sm font-medium",
            error() ? "text-error" : "text-text-primary"
          )}>
            {isLoading()
              ? "Authenticating..."
              : hasAttempted() && error()
                ? "Try Again"
                : "Authenticate with Biometrics"
            }
          </span>
        </button>

        {/* Error message */}
        <Show when={error()}>
          <div class="mt-6 flex items-start gap-3 rounded-xl bg-error/5 border border-error/10 px-4 py-3">
            <AlertCircle class="size-4 text-error mt-0.5 flex-shrink-0" />
            <p class="text-sm text-error">
              {error()}
            </p>
          </div>
        </Show>

        {/* Hint text */}
        <Show when={!hasAttempted()}>
          <p class="mt-6 text-center text-xs text-text-muted">
            Use Face ID, fingerprint, or device PIN
          </p>
        </Show>
      </div>

      {/* Subtle bottom decoration */}
      <div class="absolute bottom-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-border to-transparent" />
    </div>
  );
};

export default ApiKeyUnlock;
