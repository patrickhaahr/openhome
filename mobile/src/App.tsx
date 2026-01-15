import type { Component } from "solid-js";
import { createResource, Show, useTransition, ErrorBoundary, Suspense } from "solid-js";
import { getHealthStatus, getHealthUrl, HealthResponse } from "./api";
import "./App.css";

interface AppProps {}

const App: Component<AppProps> = () => {
  const [isPending, startTransition] = useTransition();

  const [healthData, { refetch }] = createResource(async () => {
    const url = await getHealthUrl();
    const response: HealthResponse = await getHealthStatus(url);
    return { url, status: response.status };
  });

  const handleRefresh = () => {
    startTransition(() => {
      refetch();
    });
  };

  return (
    <ErrorBoundary fallback={(err) => (
      <main class="container">
        <h1>Home App</h1>
        <p class="error">Error: {err.message}</p>
        <button onClick={() => window.location.reload()}>Reload</button>
      </main>
    )}>
      <Suspense fallback={<main class="container"><h1>Home App</h1><p>Loading...</p></main>}>
        <main class="container">
          <h1>Home App</h1>
          
          <div class="health-section">
            <h2>API Health</h2>
            <Show when={healthData.error}>
              {(err) => <p class="error">Error: {err().message}</p>}
            </Show>
            <Show when={healthData()}>
              {(data) => (
                <>
                  <p>Request URL: <strong>{data().url}</strong></p>
                  <p>Status: <strong>{data().status}</strong></p>
                </>
              )}
            </Show>
            <button onClick={handleRefresh} disabled={isPending()}>
              {isPending() ? "Refreshing..." : "Refresh"}
            </button>
          </div>
        </main>
      </Suspense>
    </ErrorBoundary>
  );
};

export default App;
