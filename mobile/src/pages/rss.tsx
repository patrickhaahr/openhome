import type { Component, SuspenseProps } from "solid-js";
import { Suspense, ErrorBoundary } from "solid-js";
import RssFeed from "../components/rss-feed";

const LoadingFallback: Component<SuspenseProps> = (props) => {
  return (
    <div class="animate-pulse space-y-4">
      <div class="h-32 rounded-lg bg-surface-secondary" />
      <div class="h-24 rounded-lg bg-surface-secondary" />
      <div class="h-24 rounded-lg bg-surface-secondary" />
    </div>
  );
};

const Rss: Component = () => {
  return (
    <div class="mx-auto max-w-lg space-y-6">
      <header class="mb-6">
        <h1 class="text-xl font-semibold text-text-primary sm:text-2xl">
          RSS Feeds
        </h1>
      </header>
      <Suspense fallback={<LoadingFallback />}>
        <ErrorBoundary fallback={(err) => (
          <div class="rounded-lg bg-error/10 px-4 py-3 text-error">
            {err.message}
          </div>
        )}>
          <RssFeed />
        </ErrorBoundary>
      </Suspense>
    </div>
  );
};

export default Rss;
