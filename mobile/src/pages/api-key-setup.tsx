import type { Component } from "solid-js";
import { createSignal, Show, createMemo, onMount } from "solid-js";
import { saveApiKey, saveSettings, baseUrl, timeoutSeconds } from "../stores/config";
import { getHealthStatus } from "../api/health";
import { Key, ArrowRight, Check, Loader2, Globe } from "lucide-solid";
import { cn } from "@/lib/utils";

interface ApiKeySetupProps {
  onValidated: () => void;
}

const ApiKeySetup: Component<ApiKeySetupProps> = (props) => {
  const [apiKey, setApiKey] = createSignal("");
  const [baseUrlInput, setBaseUrlInput] = createSignal("");
  const [status, setStatus] = createSignal<"idle" | "saving" | "validating" | "success" | "error">("idle");
  const [_errorMessage, setErrorMessage] = createSignal("");

  // Initialize base URL input with current value
  onMount(() => {
    setBaseUrlInput(baseUrl());
  });

  const isLoading = createMemo(() => status() === "saving" || status() === "validating");
  const isSuccess = createMemo(() => status() === "success");
  const isError = createMemo(() => status() === "error");
  const hasInput = createMemo(() => apiKey().trim().length > 0 && baseUrlInput().trim().length > 0);

  const handleSave = async () => {
    const key = apiKey();
    if (!key.trim()) {
      setStatus("error");
      setErrorMessage("Required");
      return;
    }

    setStatus("saving");
    setErrorMessage("");

    try {
      // Save base URL first
      await saveSettings(baseUrlInput(), timeoutSeconds());
      await saveApiKey(key);
      setStatus("validating");
      await getHealthStatus();
      setStatus("success");
      setTimeout(props.onValidated, 600);
    } catch {
      setStatus("error");
    }
  };

  return (
    <div class="relative flex min-h-screen flex-col items-center justify-center bg-bg-primary px-6 overflow-hidden">
      {/* Animated gradient orbs */}
      <div class="pointer-events-none absolute -top-40 -right-40 h-80 w-80 rounded-full bg-accent/8 blur-3xl animate-pulse" />
      <div class="pointer-events-none absolute top-1/3 -left-40 h-64 w-64 rounded-full bg-accent/5 blur-3xl" />
      <div class="pointer-events-none absolute -bottom-32 right-1/4 h-48 w-48 rounded-full bg-accent/6 blur-3xl animate-pulse" style={{ "animation-delay": "1s" }} />

      {/* Main content */}
      <div class="relative w-full max-w-sm animate-in fade-in slide-in-from-bottom-4 duration-700">
        {/* Key icon hero */}
        <div class="flex justify-center mb-12">
          <div class={cn(
            "relative size-24 rounded-3xl flex items-center justify-center transition-all duration-500",
            isSuccess() 
              ? "bg-success/10 border-2 border-success/30" 
              : isError()
                ? "bg-error/10 border-2 border-error/30"
                : "bg-bg-secondary/80 border border-border backdrop-blur-xl"
          )}>
            {/* Glow effect */}
            <div class={cn(
              "absolute inset-0 rounded-3xl blur-xl transition-all duration-500",
              isSuccess() 
                ? "bg-success/20" 
                : isError()
                  ? "bg-error/20"
                  : isLoading()
                    ? "bg-accent/20 animate-pulse"
                    : "bg-accent/10"
            )} />
            
            {/* Icon */}
            <div class="relative">
              <Show when={!isLoading() && !isSuccess()}>
                <Key class={cn(
                  "size-10 transition-colors duration-300",
                  isError() ? "text-error" : "text-accent"
                )} />
              </Show>
              <Show when={isLoading()}>
                <Loader2 class="size-10 text-accent animate-spin" />
              </Show>
              <Show when={isSuccess()}>
                <Check class="size-10 text-success animate-in zoom-in duration-300" />
              </Show>
            </div>
          </div>
        </div>

        {/* Input area */}
        <div class="space-y-3">
          {/* Base URL input */}
          <div class={cn(
            "relative rounded-2xl border bg-bg-secondary/60 backdrop-blur-sm transition-all duration-300",
            isError() 
              ? "border-error/40" 
              : isSuccess()
                ? "border-success/40"
                : baseUrlInput().trim()
                  ? "border-accent/30"
                  : "border-border"
          )}>
            <div class="absolute left-4 top-1/2 -translate-y-1/2 text-text-muted">
              <Globe class="size-4" />
            </div>
            <input
              type="url"
              value={baseUrlInput()}
              onInput={(e) => {
                setBaseUrlInput(e.currentTarget.value);
                if (isError()) setStatus("idle");
              }}
              placeholder="https://api.example.com"
              disabled={isLoading() || isSuccess()}
              class={cn(
                "w-full bg-transparent pl-11 pr-5 py-3.5 text-sm text-text-primary placeholder:text-text-muted/60",
                "focus:outline-none disabled:cursor-not-allowed disabled:opacity-60",
                "font-mono"
              )}
            />
          </div>

          {/* API Key input container */}
          <div class={cn(
            "relative rounded-2xl border bg-bg-secondary/60 backdrop-blur-sm transition-all duration-300",
            isError() 
              ? "border-error/40" 
              : isSuccess()
                ? "border-success/40"
                : apiKey().trim()
                  ? "border-accent/30"
                  : "border-border"
          )}>
            <input
              type="password"
              value={apiKey()}
              onInput={(e) => {
                setApiKey(e.currentTarget.value);
                if (isError()) setStatus("idle");
              }}
              placeholder="Secret"
              disabled={isLoading() || isSuccess()}
              onKeyDown={(e) => e.key === "Enter" && handleSave()}
              class={cn(
                "w-full bg-transparent px-5 py-4 pr-14 text-base text-text-primary placeholder:text-text-muted/60",
                "focus:outline-none disabled:cursor-not-allowed disabled:opacity-60",
                "font-mono tracking-wider"
              )}
            />
            
            {/* Submit button - icon only */}
            <button
              onClick={handleSave}
              disabled={isLoading() || isSuccess() || !hasInput()}
              class={cn(
                "absolute right-2 top-1/2 -translate-y-1/2 size-10 rounded-xl flex items-center justify-center transition-all duration-300",
                hasInput() && !isLoading() && !isSuccess()
                  ? "bg-accent text-white hover:bg-accent-hover active:scale-95"
                  : "bg-bg-tertiary text-text-muted",
                (isLoading() || isSuccess() || !hasInput()) && "opacity-40 cursor-not-allowed"
              )}
            >
              <ArrowRight class="size-5" />
            </button>
          </div>

          {/* Loading indicator */}
          <div class="h-6 flex items-center justify-center">
            <Show when={isLoading()}>
              <div class="flex gap-1 animate-in fade-in duration-300">
                <span class="size-1.5 rounded-full bg-accent animate-bounce" style={{ "animation-delay": "0ms" }} />
                <span class="size-1.5 rounded-full bg-accent animate-bounce" style={{ "animation-delay": "150ms" }} />
                <span class="size-1.5 rounded-full bg-accent animate-bounce" style={{ "animation-delay": "300ms" }} />
              </div>
            </Show>
          </div>
        </div>
      </div>

      {/* Subtle bottom decoration */}
      <div class="absolute bottom-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-border to-transparent" />
    </div>
  );
};

export default ApiKeySetup;
