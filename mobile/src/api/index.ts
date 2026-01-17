import { invoke } from "@tauri-apps/api/core";
import { baseUrl, timeoutSeconds } from "../stores/config";

interface ApiResponse {
  status: number;
  data: unknown;
}

export interface KeyringDiagnostics {
  key_present: boolean;
  key_length: number | null;
  keyring_accessible: boolean;
}

const API_METHODS = ["GET", "POST", "DELETE"] as const;
type ApiMethod = (typeof API_METHODS)[number];

interface CallApiOptions {
  path: string;
  method: ApiMethod;
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

export async function getKeyringStatus(): Promise<KeyringDiagnostics> {
  return invoke<KeyringDiagnostics>("get_keyring_diagnostics");
}
