import type { AdguardStatus } from "../domain/adguard";
import type { Configuration } from "../domain/configuration";
import {
  isJsonArray,
  isJsonBoolean,
  isJsonObject,
  isJsonNumber,
  isJsonString,
  type Json,
} from "../domain/json";
import { failure, success, type Result } from "../domain/result";

/** Status and available command sets returned by the IR API. */
export type IrStatus = {
  readonly message: string;
  readonly edifierCommands: ReadonlySet<string>;
  readonly lgTvCommands: ReadonlySet<string>;
};

/** AdGuard Protection operations used by the Server Tab. */
export type AdguardApi = {
  readonly getStatus: () => Promise<Result<AdguardStatus>>;
  readonly enableProtection: () => Promise<Result<AdguardStatus>>;
  readonly disableProtection: () => Promise<Result<AdguardStatus>>;
  readonly pauseProtection: (minutes: number) => Promise<Result<AdguardStatus>>;
};

/** Operations used by the OpenHome application layer. */
export type OpenHomeApi = {
  readonly validateConfiguration: () => Promise<Result<void>>;
  readonly getIrStatus: () => Promise<Result<IrStatus>>;
  readonly sendIrCommand: (remote: "edifier" | "lgtv", command: string) => Promise<Result<void>>;
  readonly sendLightCommand: (command: "on" | "off") => Promise<Result<void>>;
  readonly adguard: AdguardApi;
};

type RequestOptions = {
  readonly method?: "GET" | "POST";
  readonly body?: object;
  readonly defaultError: string;
};

/** Headers sent with every OpenHome API request. */
type RequestHeaders = {
  Authorization: string;
  "Content-Type"?: string;
};

type ApiResponse = { readonly body: string };

/** Create an HTTP adapter scoped to one validated configuration. */
export function createOpenHomeApi(configuration: Configuration): OpenHomeApi {
  async function adguardStatus(
    path: string,
    options: RequestOptions,
  ): Promise<Result<AdguardStatus>> {
    return parseAdguardResponse(await request(path, options));
  }

  async function request(path: string, options: RequestOptions): Promise<Result<ApiResponse>> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 5_000);

    try {
      const headers: RequestHeaders = {
        Authorization: `Bearer ${configuration.apiKey}`,
      };
      if (options.body !== undefined) {
        headers["Content-Type"] = "application/json";
      }
      const response = await fetch(`${configuration.baseUrl}${path}`, {
        method: options.method ?? "GET",
        headers,
        body: options.body === undefined ? undefined : JSON.stringify(options.body),
        redirect: "manual",
        signal: controller.signal,
      });

      const body = await response.text();
      if (!response.ok) {
        return failure(readError(body, options.defaultError));
      }
      return success({ body });
    } catch {
      return failure("Couldn't reach the Axum API. Check the Base URL and try again.");
    } finally {
      clearTimeout(timeout);
    }
  }

  return {
    async validateConfiguration(): Promise<Result<void>> {
      const response = await request("/api/health", {
        defaultError: "OpenHome rejected that Base URL or API Key.",
      });
      return response.ok ? success(undefined) : response;
    },

    async getIrStatus(): Promise<Result<IrStatus>> {
      const response = await request("/api/ir", {
        defaultError: "Couldn't load IR status from the Axum API.",
      });
      if (!response.ok) {
        return response;
      }

      try {
        return parseIrStatus(JSON.parse(response.value.body));
      } catch {
        return failure("Couldn't read IR status from the Axum API.");
      }
    },

    async sendIrCommand(remote: "edifier" | "lgtv", command: string): Promise<Result<void>> {
      const response = await request(`/api/ir/${remote}`, {
        method: "POST",
        body: { command },
        defaultError: "Couldn't send that IR command to the Axum API.",
      });
      return response.ok ? success(undefined) : response;
    },

    async sendLightCommand(command: "on" | "off"): Promise<Result<void>> {
      const response = await request(`/api/lights/${command}`, {
        method: "POST",
        defaultError: "Couldn't switch the light.",
      });
      return response.ok ? success(undefined) : response;
    },

    adguard: {
      getStatus: () => adguardStatus("/api/adguard/status", { defaultError: ADGUARD_READ_ERROR }),
      enableProtection: () =>
        adguardStatus("/api/adguard/enable", {
          method: "POST",
          defaultError: "Couldn't enable AdGuard Protection.",
        }),
      disableProtection: () =>
        adguardStatus("/api/adguard/disable", {
          method: "POST",
          defaultError: "Couldn't disable AdGuard Protection.",
        }),
      pauseProtection: (minutes: number) =>
        adguardStatus("/api/adguard/pause", {
          method: "POST",
          body: { minutes },
          defaultError: "Couldn't pause AdGuard Protection.",
        }),
    },
  };
}

/** Parse an untrusted IR status response. */
export function parseIrStatus(json: Json): Result<IrStatus> {
  if (!isJsonObject(json)) {
    return failure("Couldn't read IR status from the Axum API.");
  }
  const remotes = json["remotes"];
  if (!isJsonObject(remotes)) {
    return failure("Couldn't read IR status from the Axum API.");
  }
  const edifier = remotes["edifier"];
  const lgTv = remotes["lgtv"];
  if (!isCommandList(edifier) || !isCommandList(lgTv)) {
    return failure("Couldn't read IR status from the Axum API.");
  }

  const message = json["message"];
  const trimmedMessage = isJsonString(message) ? message.trim() : "";
  return success({
    message: trimmedMessage.length > 0 ? trimmedMessage : "IR remote ready",
    edifierCommands: new Set(edifier.map((command) => command.trim()).filter(Boolean)),
    lgTvCommands: new Set(lgTv.map((command) => command.trim()).filter(Boolean)),
  });
}

function isCommandList(value: Json | undefined): value is readonly string[] {
  return isJsonArray(value) && value.every(isJsonString);
}

const ADGUARD_READ_ERROR = "Couldn't read AdGuard status from the Axum API.";

/** Parse an untrusted AdGuard status response, resolving pause timing against nowMs. */
export function parseAdguardStatus(json: Json, nowMs: number): Result<AdguardStatus> {
  if (!isJsonObject(json)) {
    return failure(ADGUARD_READ_ERROR);
  }
  const version = json["version"];
  const running = json["running"];
  const protectionEnabled = json["protection_enabled"];
  if (!isJsonString(version) || !isJsonBoolean(running) || !isJsonBoolean(protectionEnabled)) {
    return failure(ADGUARD_READ_ERROR);
  }

  const duration = json["protection_disabled_duration"];
  const until = json["protection_disabled_until"];
  const durationMs = isJsonNumber(duration) && duration > 0 ? duration : 0;
  const untilMs = isJsonString(until) ? Date.parse(until) : Number.NaN;
  const pauseEndsAtMs = Number.isFinite(untilMs)
    ? untilMs
    : durationMs > 0
      ? nowMs + durationMs
      : null;
  return success({ version, running, protectionEnabled, pauseEndsAtMs });
}

/** Apply the AdGuard status shape guard to a raw response body. */
function parseAdguardResponse(response: Result<{ readonly body: string }>): Result<AdguardStatus> {
  if (!response.ok) {
    return response;
  }
  try {
    return parseAdguardStatus(JSON.parse(response.value.body), Date.now());
  } catch {
    return failure(ADGUARD_READ_ERROR);
  }
}

function readError(body: string, fallback: string): string {
  let json: Json;
  try {
    json = JSON.parse(body);
  } catch {
    return fallback;
  }
  if (!isJsonObject(json)) {
    return fallback;
  }
  const error = json["error"];
  const trimmedError = isJsonString(error) ? error.trim() : "";
  return trimmedError.length > 0 ? trimmedError : fallback;
}
