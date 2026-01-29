import type { Component } from "solid-js";
import { createMemo, createSignal, onMount, Show, ErrorBoundary, onCleanup } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Docker from "./pages/docker";
import BottomNav, { type NavPage } from "./components/ui/bottom-nav";
import SwipeablePages from "./components/ui/swipeable-pages";
import FactCard from "./components/fact-card";
import ApiStatusIndicator from "./components/ui/api-status-indicator";
import AdguardControl from "./components/adguard-control";
import DockerHealth from "./components/docker-health";
import RssFeed from "./components/rss-feed";
import ApiKeySetup from "./pages/api-key-setup";
import ApiKeyUnlock from "./pages/api-key-unlock";
import { auth } from "@/stores/auth";

const PAGES: NavPage[] = ["home", "docker"];

const App: Component = () => {
  const [pageIndex, setPageIndex] = createSignal(0);
  const [isNavVisible, setIsNavVisible] = createSignal(true);

  const currentPage = createMemo(() => PAGES[pageIndex()]);

  const handleScrollDirection = (direction: "up" | "down") => {
    setIsNavVisible(direction === "up");
  };

  const handleNavigate = (page: NavPage) => {
    const index = PAGES.indexOf(page);
    if (index !== -1) {
      setPageIndex(index);
    }
  };

  const isHomePage = createMemo(() => currentPage() === "home");

  // Initialize auth on mount
  onMount(async () => {
    await auth.loadStatus();
  });

  // Visibility change lock listener with debounce
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const handleVisibilityChange = () => {
    if (document.visibilityState === "hidden") {
      if (debounceTimer) clearTimeout(debounceTimer);
      debounceTimer = setTimeout(async () => {
        await invoke("clear_api_key_cache");
        await auth.loadStatus();
      }, 500);
    } else {
      if (debounceTimer) clearTimeout(debounceTimer);
    }
  };

  onMount(() => {
    document.addEventListener("visibilitychange", handleVisibilityChange);
  });

  onCleanup(() => {
    document.removeEventListener("visibilitychange", handleVisibilityChange);
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  // Auth resume event listener
  const handleAuthResume = async () => {
    // The unlock screen handles status reload on successful auth
    // Routing will show unlock screen if locked based on current auth status
  };

  let unlistenAuthResume: (() => void) | null = null;

  onMount(async () => {
    unlistenAuthResume = await listen("auth:resume", handleAuthResume);
  });

  onCleanup(() => {
    if (unlistenAuthResume) unlistenAuthResume();
  });

  return (
    <>
      {/* NotSet -> setup screen */}
      <Show when={auth.notSet()}>
        <ApiKeySetup onValidated={() => { /* auth store already updated */ }} />
      </Show>

      {/* Locked -> unlock screen */}
      <Show when={auth.isLocked()}>
        <ApiKeyUnlock />
      </Show>

      {/* Unlocked -> main UI */}
      <Show when={auth.isUnlocked()}>
        <div class="relative flex min-h-screen flex-col bg-bg-primary overflow-hidden">
          {/* Subtle gradient orb background */}
          <div class="pointer-events-none absolute -top-32 -right-32 h-64 w-64 rounded-full bg-accent/5 blur-3xl" />
          <div class="pointer-events-none absolute top-1/2 -left-32 h-48 w-48 rounded-full bg-accent/3 blur-3xl" />

          {/* API Status Indicator - fixed top right */}
          <Show when={isHomePage()}>
            <div class="fixed right-4 top-12 z-50">
              <ApiStatusIndicator />
            </div>
          </Show>

          <main class="relative flex-1 pt-24 pb-24">
            <SwipeablePages currentIndex={pageIndex} onIndexChange={setPageIndex} onScrollDirectionChange={handleScrollDirection}>
              {[
                /* Home Page */
                <div class="px-5 pt-8 pb-4">
                  <div class="mx-auto max-w-md">
                    <ErrorBoundary fallback={(err) => (
                      <div class="rounded-2xl bg-error/5 border border-error/10 px-4 py-3 text-error text-sm">
                        {err.message}
                      </div>
                    )}>
                      <div class="space-y-6">
                        {/* Fact section - hero area */}
                        <section class="pt-2">
                          <FactCard />
                        </section>

                        {/* Status row - AdGuard + Docker health */}
                        <section class="space-y-3">
                          <AdguardControl />
                          <DockerHealth onClick={() => setPageIndex(1)} />
                        </section>

                        {/* RSS Feed - scrollable timeline */}
                        <section class="pt-2">
                          <RssFeed />
                        </section>
                      </div>
                    </ErrorBoundary>
                  </div>
                </div>,

                /* Docker Page */
                <div class="px-5 pb-4">
                  <div class="mx-auto max-w-md">
                    <ErrorBoundary fallback={(err) => (
                      <div class="rounded-2xl bg-error/5 border border-error/10 px-4 py-3 text-error text-sm">
                        {err.message}
                      </div>
                    )}>
                      <Docker />
                    </ErrorBoundary>
                  </div>
                </div>
              ]}
            </SwipeablePages>
          </main>

          <BottomNav currentPage={currentPage} onNavigate={handleNavigate} visible={isNavVisible} />
        </div>
      </Show>
    </>
  );
};

export default App;
