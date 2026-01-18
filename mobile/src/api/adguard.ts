import { callApi } from "./client";

export interface AdguardStatus {
  protection_enabled: boolean;
  protection_disabled_duration: number;
  protection_disabled_until: string | null;
  version: string;
  running: boolean;
}

export async function getAdguardStatus(): Promise<AdguardStatus> {
  const response = await callApi({
    path: "/api/adguard/status",
    method: "GET",
  });

  if (response.status >= 200 && response.status < 300) {
    if (
      typeof response.data !== "object" ||
      response.data === null ||
      !("protection_enabled" in response.data) ||
      !("running" in response.data)
    ) {
      throw new Error("Invalid AdGuard status response shape: missing required fields");
    }
    return response.data as AdguardStatus;
  }

  if (response.status === 503) {
    throw new Error("AdGuard is not configured on the server");
  }

  throw new Error(`Failed to fetch AdGuard status: Status ${response.status}`);
}

export async function enableProtection(): Promise<AdguardStatus> {
  const response = await callApi({
    path: "/api/adguard/enable",
    method: "POST",
  });

  if (response.status >= 200 && response.status < 300) {
    if (
      typeof response.data !== "object" ||
      response.data === null ||
      !("protection_enabled" in response.data) ||
      !("running" in response.data)
    ) {
      throw new Error("Invalid AdGuard status response shape: missing required fields");
    }
    return response.data as AdguardStatus;
  }

  throw new Error(`Failed to enable AdGuard protection: Status ${response.status}`);
}

export async function disableProtection(): Promise<AdguardStatus> {
  const response = await callApi({
    path: "/api/adguard/disable",
    method: "POST",
  });

  if (response.status >= 200 && response.status < 300) {
    if (
      typeof response.data !== "object" ||
      response.data === null ||
      !("protection_enabled" in response.data) ||
      !("running" in response.data)
    ) {
      throw new Error("Invalid AdGuard status response shape: missing required fields");
    }
    return response.data as AdguardStatus;
  }

  throw new Error(`Failed to disable AdGuard protection: Status ${response.status}`);
}

export async function pauseProtection(minutes: number): Promise<AdguardStatus> {
  const response = await callApi({
    path: "/api/adguard/pause",
    method: "POST",
    body: { minutes },
  });

  if (response.status >= 200 && response.status < 300) {
    if (
      typeof response.data !== "object" ||
      response.data === null ||
      !("protection_enabled" in response.data) ||
      !("running" in response.data)
    ) {
      throw new Error("Invalid AdGuard status response shape: missing required fields");
    }
    return response.data as AdguardStatus;
  }

  throw new Error(`Failed to pause AdGuard protection: Status ${response.status}`);
}
