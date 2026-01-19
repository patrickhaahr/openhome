import { callApi } from "./client";

export interface DockerContainerStatus {
  name: string;
  status: string;
  state: string;
  health_status: string | null;
  uptime_seconds: number | null;
  image: string;
  ports: string[];
  labels: Record<string, string>;
  created_at: string;
  restart_count: number;
}

export interface DockerContainerListResponse {
  containers: DockerContainerStatus[];
  timestamp: string;
}

export interface DockerRestartResponse {
  success: boolean;
  message: string;
}

export interface DockerStartResponse {
  success: boolean;
  message: string;
}

export interface DockerStopResponse {
  success: boolean;
  message: string;
  stopped: boolean;
}

export interface DockerLogsParams {
  tail?: number;
  since?: string;
  timestamps?: boolean;
}

const parseContainer = (raw: Record<string, unknown>): DockerContainerStatus => {
  const healthStatus = raw.HealthStatus ?? raw.health_status;
  const createdAt = raw.Created ?? raw.created_at;

  const labels = typeof raw.labels === "object" && raw.labels !== null
    ? (raw.labels as Record<string, string>)
    : {};

  const uptimeSeconds = typeof raw.uptime_seconds === "number"
    ? raw.uptime_seconds
    : typeof raw.uptime_seconds === "string"
      ? Number.parseFloat(raw.uptime_seconds)
      : null;

  return {
    name: typeof raw.name === "string" ? raw.name : "",
    status: typeof raw.status === "string" ? raw.status : "",
    state: typeof raw.state === "string" ? raw.state : "",
    health_status: typeof healthStatus === "string" ? healthStatus : null,
    uptime_seconds: Number.isFinite(uptimeSeconds) ? uptimeSeconds : null,
    image: typeof raw.image === "string" ? raw.image : "",
    ports: Array.isArray(raw.ports) ? raw.ports.map(String) : [],
    labels,
    created_at: typeof createdAt === "string" ? createdAt : "",
    restart_count: typeof raw.restart_count === "number" ? raw.restart_count : 0,
  };
};

export async function listDockerContainers(): Promise<DockerContainerListResponse> {
  const response = await callApi({
    path: "/api/docker",
    method: "GET",
  });

  if (response.status >= 200 && response.status < 300) {
    if (
      typeof response.data !== "object" ||
      response.data === null ||
      !("containers" in response.data)
    ) {
      throw new Error("Invalid Docker list response shape: missing containers");
    }

    const data = response.data as { containers: unknown[]; timestamp: string };
    if (!Array.isArray(data.containers)) {
      throw new Error("Invalid Docker list response shape: containers is not an array");
    }

    const containers = data.containers
      .filter((container) => typeof container === "object" && container !== null)
      .map((container) => parseContainer(container as Record<string, unknown>));

    return {
      containers,
      timestamp: typeof data.timestamp === "string" ? data.timestamp : "",
    };
  }

  if (response.status === 503) {
    throw new Error("Docker service is not available");
  }

  throw new Error(`Failed to fetch Docker containers: Status ${response.status}`);
}

export async function getDockerLogs(
  containerName: string,
  params: DockerLogsParams
): Promise<string> {
  const query = new URLSearchParams();

  if (params.tail !== undefined) {
    query.set("tail", String(params.tail));
  }
  if (params.since) {
    query.set("since", params.since);
  }
  if (params.timestamps !== undefined) {
    query.set("timestamps", params.timestamps ? "true" : "false");
  }

  const response = await callApi({
    path: `/api/docker/${encodeURIComponent(containerName)}/logs?${query.toString()}`,
    method: "GET",
  });

  if (response.status >= 200 && response.status < 300) {
    if (typeof response.data !== "string") {
      throw new Error("Invalid Docker logs response shape: expected string");
    }
    return response.data;
  }

  if (response.status === 404) {
    throw new Error(`Container "${containerName}" not found`);
  }

  throw new Error(`Failed to fetch logs: Status ${response.status}`);
}

export async function restartDockerContainer(
  containerName: string,
  timeoutSeconds: number
): Promise<DockerRestartResponse> {
  const response = await callApi({
    path: `/api/docker/${encodeURIComponent(containerName)}/restart`,
    method: "POST",
    body: { timeout_seconds: timeoutSeconds },
  });

  if (response.status >= 200 && response.status < 300) {
    if (
      typeof response.data !== "object" ||
      response.data === null ||
      !("success" in response.data)
    ) {
      throw new Error("Invalid restart response shape: missing success");
    }
    return response.data as DockerRestartResponse;
  }

  if (response.status === 404) {
    throw new Error(`Container "${containerName}" not found`);
  }

  throw new Error(`Failed to restart container: Status ${response.status}`);
}

export async function startDockerContainer(
  containerName: string
): Promise<DockerStartResponse> {
  const response = await callApi({
    path: `/api/docker/${encodeURIComponent(containerName)}/start`,
    method: "POST",
  });

  if (response.status >= 200 && response.status < 300) {
    if (
      typeof response.data !== "object" ||
      response.data === null ||
      !("success" in response.data)
    ) {
      throw new Error("Invalid start response shape: missing success");
    }
    return response.data as DockerStartResponse;
  }

  if (response.status === 404) {
    throw new Error(`Container "${containerName}" not found`);
  }

  if (response.status === 503) {
    throw new Error("Docker service is not available");
  }

  throw new Error(`Failed to start container: Status ${response.status}`);
}

export async function stopDockerContainer(
  containerName: string,
  timeoutSeconds: number
): Promise<DockerStopResponse> {
  const response = await callApi({
    path: `/api/docker/${encodeURIComponent(containerName)}/stop`,
    method: "POST",
    body: { timeout_seconds: timeoutSeconds },
  });

  if (response.status >= 200 && response.status < 300) {
    if (
      typeof response.data !== "object" ||
      response.data === null ||
      !("success" in response.data)
    ) {
      throw new Error("Invalid stop response shape: missing success");
    }
    return response.data as DockerStopResponse;
  }

  if (response.status === 404) {
    throw new Error(`Container "${containerName}" not found`);
  }

  if (response.status === 503) {
    throw new Error("Docker service is not available");
  }

  throw new Error(`Failed to stop container: Status ${response.status}`);
}
