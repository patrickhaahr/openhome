import type { Component } from "solid-js";
import { createResource, Show, useTransition, ErrorBoundary, Suspense, createSignal } from "solid-js";
import { getHealthStatus, getHealthUrl } from "./api";
import { baseUrl } from "./stores/config";
import Settings from "./pages/settings";
import BottomNav, { type NavPage } from "./components/ui/bottom-nav";

const App: Component = () => {
  const [isPending, startTransition] = useTransition();
  const [currentPage, setCurrentPage] = createSignal<NavPage>("home");
  
  // Resource tracks baseUrl changes automatically if accessed
  const [healthData, { refetch }] = createResource(baseUrl, async (currentUrl) => {
    if (!currentUrl) return null;
    try {
      const response = await getHealthStatus();
      // For display purposes, get the display URL
      const displayUrl = await getHealthUrl();
      return { url: displayUrl, status: response.status };
    } catch (e: unknown) {
      throw e;
    }
  });

  const handleRefresh = () => {
    startTransition(() => {
      refetch();
    });
  };

  const handleNavigate = (page: NavPage) => {
    setCurrentPage(page);
  };

  return (
    <div class="flex min-h-screen flex-col bg-bg-primary">
      <main class="flex-1 px-4 py-6 pb-20 sm:px-6 lg:px-8">
        <div class="mx-auto max-w-lg">
          <header class="mb-6">
            <h1 class="text-xl font-semibold text-text-primary sm:text-2xl">Home App</h1>
          </header>

          <Show when={currentPage() === "settings"} fallback={
            <ErrorBoundary fallback={(err) => (
              <p class="rounded-lg bg-error/10 px-4 py-3 text-error">{err.message}</p>
            )}>
              <Suspense fallback={
                <p class="text-text-secondary">Loading...</p>
              }>
                <div class="rounded-xl bg-bg-secondary p-4 sm:p-6">
                  <h2 class="mb-4 text-lg font-medium text-text-primary">API Health</h2>
                  
                  <Show when={healthData.error}>
                    {(err) => (
                      <p class="mb-4 rounded-lg bg-error/10 px-4 py-3 text-sm text-error">
                        Connection Failed: {err().message}
                      </p>
                    )}
                  </Show>
                  
                  <Show when={healthData()} fallback={
                    <p class="text-text-secondary">No configuration set</p>
                  }>
                    {(data) => (
                      <div class="mb-4 space-y-2">
                        <p class="text-sm text-text-secondary">
                          Request URL: <span class="font-medium text-text-primary break-all">{data().url}</span>
                        </p>
                        <p class="text-sm text-text-secondary">
                          Status: <span class="font-medium text-success">{data().status}</span>
                        </p>
                      </div>
                    )}
                  </Show>
                  
                  <button 
                    onClick={handleRefresh} 
                    disabled={isPending()} 
                    class="w-full rounded-lg bg-accent px-4 py-3 font-medium text-white transition-colors hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-50 sm:w-auto"
                  >
                    {isPending() ? "Refreshing..." : "Refresh"}
                  </button>
                </div>
              </Suspense>
            </ErrorBoundary>
          }>
            <Settings onSaved={refetch} />
          </Show>
        </div>
      </main>
      
      <BottomNav currentPage={currentPage()} onNavigate={handleNavigate} />
    </div>
  );
};

export default App;
