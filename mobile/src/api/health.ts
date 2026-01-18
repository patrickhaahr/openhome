import { callApi } from "./client";
import { baseUrl } from "../stores/config";

export interface HealthResponse {
  status: string;
}

export async function getHealthUrl(): Promise<string> {
  const url = baseUrl();
  return `${url}/api/health`;
}

export async function getHealthStatus(apiKeyOverride?: string): Promise<HealthResponse> {
  const response = await callApi({
    path: "/api/health",
    method: "GET",
    apiKeyOverride,
  });

  if (response.status >= 200 && response.status < 300) {
    return response.data as HealthResponse;
  } else {
    throw new Error(`Health check failed: Status ${response.status}`);
  }
}
