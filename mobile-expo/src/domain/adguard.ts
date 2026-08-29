/** Protection state of the homelab's AdGuard instance, as served by the Axum API. */
export type AdguardStatus = {
  readonly version: string;
  readonly running: boolean;
  readonly protectionEnabled: boolean;
  /** Epoch ms at which a Protection Pause ends, or null when no timed pause is active. */
  readonly pauseEndsAtMs: number | null;
};

/** The user-visible phases of AdGuard Protection. */
export type ProtectionPhase = "protected" | "paused" | "unprotected";

/** Classify the protection phase a status represents at a point in time. */
export function protectionPhase(status: AdguardStatus, nowMs: number): ProtectionPhase {
  if (status.protectionEnabled) {
    return "protected";
  }
  return status.pauseEndsAtMs !== null && status.pauseEndsAtMs > nowMs ? "paused" : "unprotected";
}

/** Render remaining pause time as compact countdown text, clamped to zero. */
export function formatPauseRemaining(remainingMs: number): string {
  const totalSeconds = Math.max(0, Math.ceil(remainingMs / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`;
}

/** Pause durations offered on the Server Tab, in minutes. */
export const pauseOptionsMinutes: readonly number[] = [5, 15, 30, 60];
