import type { Component } from "solid-js";
import { createSignal, Show, ErrorBoundary } from "solid-js";
import Settings from "./pages/settings";
import Rss from "./pages/rss";
import BottomNav, { type NavPage } from "./components/ui/bottom-nav";
import FactCard from "./components/fact-card";
import ApiStatus from "./components/ui/api-status";
import AdguardControl from "./components/adguard-control";

const App: Component = () => {
  const [currentPage, setCurrentPage] = createSignal<NavPage>("home");

  const handleNavigate = (page: NavPage) => {
    setCurrentPage(page);
  };

  return (
    <div class="flex min-h-screen flex-col bg-bg-primary">
      <main class="flex-1 px-4 py-6 pb-20 sm:px-6 lg:px-8">
        <div class="mx-auto max-w-lg">
          <header class="mb-6">
            <h1 class="text-xl font-semibold text-text-primary sm:text-2xl">
              Home App
            </h1>
          </header>

          <Show when={currentPage() === "home"}>
            <ErrorBoundary fallback={(err) => (
              <p class="rounded-lg bg-error/10 px-4 py-3 text-error">
                {err.message}
              </p>
            )}>
              <div class="space-y-6">
                <FactCard />
                <ApiStatus />
                <AdguardControl />
              </div>
            </ErrorBoundary>
          </Show>

          <Show when={currentPage() === "rss"}>
            <ErrorBoundary fallback={(err) => (
              <p class="rounded-lg bg-error/10 px-4 py-3 text-error">
                {err.message}
              </p>
            )}>
              <Rss />
            </ErrorBoundary>
          </Show>

          <Show when={currentPage() === "settings"}>
            <ErrorBoundary fallback={(err) => (
              <p class="rounded-lg bg-error/10 px-4 py-3 text-error">
                {err.message}
              </p>
            )}>
              <Settings onSaved={() => {}} />
            </ErrorBoundary>
          </Show>
        </div>
      </main>

      <BottomNav currentPage={currentPage()} onNavigate={handleNavigate} />
    </div>
  );
};

export default App;
