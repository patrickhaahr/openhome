import type { Component } from "solid-js";
import { createMemo, createSignal, onMount, Show, ErrorBoundary } from "solid-js";
import Docker from "./pages/docker";
import BottomNav, { type NavPage } from "./components/ui/bottom-nav";
import FactCard from "./components/fact-card";
import ApiStatusIndicator from "./components/ui/api-status-indicator";
import AdguardControl from "./components/adguard-control";
import DockerHealth from "./components/docker-health";
import RssFeed from "./components/rss-feed";
import ApiKeySetup from "./pages/api-key-setup";
import { getKeyringStatus } from "./api/client";
import { isLoaded } from "./stores/config";

const App: Component = () => {
  const [currentPage, setCurrentPage] = createSignal<NavPage>("home");
  const [isApiKeyValid, setIsApiKeyValid] = createSignal(false);
  const [isCheckingKey, setIsCheckingKey] = createSignal(true);

  onMount(async () => {
    if (isLoaded()) {
      try {
        const status = await getKeyringStatus();
        setIsApiKeyValid(status.key_present);
      } catch {
        setIsApiKeyValid(false);
      }
    }
    setIsCheckingKey(false);
  });

  const handleNavigate = (page: NavPage) => {
    setCurrentPage(page);
  };

  const handleApiKeyValidated = () => {
    setIsApiKeyValid(true);
  };

  const isHomePage = createMemo(() => currentPage() === "home");

  return (
    <Show when={!isCheckingKey()}>
      <Show when={!isApiKeyValid()} fallback={
        <div class="relative flex min-h-screen flex-col bg-bg-primary overflow-hidden">
          {/* Subtle gradient orb background */}
          <div class="pointer-events-none absolute -top-32 -right-32 h-64 w-64 rounded-full bg-accent/5 blur-3xl" />
          <div class="pointer-events-none absolute top-1/2 -left-32 h-48 w-48 rounded-full bg-accent/3 blur-3xl" />
          
          {/* API Status Indicator - fixed top right */}
          <Show when={isHomePage()}>
            <div class="fixed right-4 top-4 z-50">
              <ApiStatusIndicator />
            </div>
          </Show>

          <main class="relative flex-1 px-5 pt-8 pb-24">
            <div class="mx-auto max-w-md">
              <Show when={currentPage() === "home"}>
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
                      <DockerHealth />
                    </section>
                    
                    {/* RSS Feed - scrollable timeline */}
                    <section class="pt-2">
                      <RssFeed />
                    </section>
                  </div>
                </ErrorBoundary>
              </Show>

              <Show when={currentPage() === "docker"}>
                <ErrorBoundary fallback={(err) => (
                  <div class="rounded-2xl bg-error/5 border border-error/10 px-4 py-3 text-error text-sm">
                    {err.message}
                  </div>
                )}>
                  <Docker />
                </ErrorBoundary>
              </Show>
            </div>
          </main>

          <BottomNav currentPage={currentPage} onNavigate={handleNavigate} />
        </div>
      }>
        <ApiKeySetup onValidated={handleApiKeyValidated} />
      </Show>
    </Show>
  );
};

export default App;
