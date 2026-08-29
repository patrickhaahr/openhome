/** One Docker workload reported by the Axum API. */
export type DockerContainer = {
  readonly name: string;
  /** Docker run state, e.g. "running" or "exited". */
  readonly state: string;
  /** Docker health check verdict, or null when the image defines none. */
  readonly health: string | null;
  readonly image: string;
  readonly uptimeSeconds: number | null;
  readonly ports: readonly string[];
  readonly restartCount: number;
};

/** The health classification a container falls into for filters and counts. */
export type ContainerClassification = "healthy" | "unhealthy" | "stopped";

/** Per-classification container counts across the whole list. */
export type ClassificationCounts = {
  readonly all: number;
  readonly healthy: number;
  readonly unhealthy: number;
  readonly stopped: number;
};

/** The Docker Health Summary headline when the Axum API is reachable. */
export type DockerSummaryClassification = "healthy" | "unhealthy" | "idle";

/** Classify one container for filtering and counting. */
export function classifyContainer(container: DockerContainer): ContainerClassification {
  if (container.state.toLowerCase() !== "running") {
    return "stopped";
  }
  return container.health?.toLowerCase() === "unhealthy" ? "unhealthy" : "healthy";
}

/** Count containers per health classification. */
export function classificationCounts(
  containers: readonly DockerContainer[],
): ClassificationCounts {
  const counts = { all: containers.length, healthy: 0, unhealthy: 0, stopped: 0 };
  for (const container of containers) {
    counts[classifyContainer(container)] += 1;
  }
  return counts;
}

/** Classify the whole stack for the Docker Health Summary card. */
export function dockerHealthSummary(
  containers: readonly DockerContainer[],
): DockerSummaryClassification {
  const counts = classificationCounts(containers);
  if (counts.healthy + counts.unhealthy === 0) {
    return "idle";
  }
  return counts.unhealthy > 0 ? "unhealthy" : "healthy";
}

/** Render container uptime as compact text, or "Unknown uptime". */
export function formatUptime(seconds: number | null): string {
  if (seconds === null) {
    return "Unknown uptime";
  }
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes}m`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}h ${minutes % 60}m`;
  }
  const days = Math.floor(hours / 24);
  return `${days}d ${hours % 24}h`;
}

/** Render a container's port mappings as one line, or "No ports exposed". */
export function formatPorts(ports: readonly string[]): string {
  return ports.length > 0 ? ports.join(", ") : "No ports exposed";
}
