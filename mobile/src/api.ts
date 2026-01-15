import { fetch } from "@tauri-apps/plugin-http";
import { invoke } from "@tauri-apps/api/core";
import { baseUrl, apiKey, ApiConfig } from "./stores/config";

export interface HealthResponse {
  status: string;
}

const resolveBaseUrl = async (): Promise<string> => {
  let url = baseUrl();

  if (!url) {
    try {
      const config = await invoke<ApiConfig>("get_api_config");
      url = config.base_url;
    } catch (e) {
      throw new Error("Failed to load API configuration");
    }
  }

  return url;
};

export async function getHealthUrl(): Promise<string> {
  const url = await resolveBaseUrl();

  return `${url}/api/health`;
}

export async function getHealthStatus(healthUrl?: string): Promise<HealthResponse> {
  const url = healthUrl ?? (await getHealthUrl());
  const key = apiKey();
  const headers: Record<string, string> = {};
  if (key) {
    headers["X-API-Key"] = key;
  }

  const response = await fetch(url, {
    method: "GET",
    headers,
  });

  if (!response.ok) {
    throw new Error(`Health check failed: ${response.statusText}`);
  }

  return (await response.json()) as HealthResponse;
}
