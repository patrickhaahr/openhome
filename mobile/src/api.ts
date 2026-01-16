import { invoke } from "@tauri-apps/api/core";
import { baseUrl, timeoutSeconds } from "./stores/config";

export interface HealthResponse {
  status: string;
}

interface ApiResponse {
  status: number;
  data: unknown;
}

export interface KeyringDiagnostics {
  key_present: boolean;
  key_length: number | null;
  keyring_accessible: boolean;
}

export async function getHealthUrl(): Promise<string> {
  const url = baseUrl();
  return `${url}/api/health`;
}

interface CallApiOptions {
  path: string;
  method: "GET" | "POST";
  body?: unknown;
  apiKeyOverride?: string;
}

export async function callApi(options: CallApiOptions): Promise<ApiResponse> {
  const url = baseUrl();
  const timeout = timeoutSeconds();

  if (!url) {
    throw new Error("Base URL not set");
  }

  const params: Record<string, unknown> = {
    path: options.path,
    method: options.method,
    body: options.body ?? null,
    baseUrl: url,
    timeoutSeconds: timeout,
    apiKeyOverride: null,
  };

  if (import.meta.env.DEV && options.apiKeyOverride) {
    params.apiKeyOverride = options.apiKeyOverride;
  }

  return invoke<ApiResponse>("call_api", params);
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

export async function getKeyringStatus(): Promise<KeyringDiagnostics> {
  return invoke<KeyringDiagnostics>("get_keyring_diagnostics");
}

export interface FactResponse {
  text: string;
}

export async function getRandomFact(): Promise<FactResponse> {
  const response = await callApi({
    path: "/api/facts/random",
    method: "GET",
  });

  if (response.status >= 200 && response.status < 300) {
    return response.data as FactResponse;
  } else {
    throw new Error(`Failed to fetch fact: Status ${response.status}`);
  }
}
