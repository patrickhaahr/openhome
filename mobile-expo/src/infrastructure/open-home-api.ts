import type { AdguardStatus } from "../domain/adguard";
import type { Configuration } from "../domain/configuration";
import type { DockerContainer } from "../domain/docker";
import type { TimelineItem } from "../domain/rss";
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

/** Docker container operations used by the Docker Tab. */
export type DockerApi = {
  readonly listContainers: () => Promise<Result<readonly DockerContainer[]>>;
  readonly containerLogs: (name: string) => Promise<Result<readonly string[]>>;
  readonly startContainer: (name: string) => Promise<Result<void>>;
  readonly stopContainer: (name: string) => Promise<Result<void>>;
  readonly restartContainer: (name: string) => Promise<Result<void>>;
};

/** Compact timeline operations used by the Server Tab. */
export type RssApi = {
  /** One newest-first page of compact feed items, resuming after `beforeId` when set. */
  readonly compactTimeline: (
    beforeId: number | null,
    limit: number,
  ) => Promise<Result<readonly TimelineItem[]>>;
};

/** Operations used by the OpenHome application layer. */
export type OpenHomeApi = {
  readonly validateConfiguration: () => Promise<Result<void>>;
  readonly getIrStatus: () => Promise<Result<IrStatus>>;
  readonly sendIrCommand: (remote: "edifier" | "lgtv", command: string) => Promise<Result<void>>;
  readonly sendLightCommand: (command: "on" | "off") => Promise<Result<void>>;
  readonly adguard: AdguardApi;
  readonly docker: DockerApi;
  readonly rss: RssApi;
};

type RequestOptions = {
  readonly method?: "GET" | "POST";
  readonly body?: object;
  readonly defaultError: string;
  /** Request timeout in ms; defaults to the short timeout shared by all callers. */
  readonly timeoutMs?: number;
  /** Status-specific error overrides for classed failures, e.g. 404 not found. */
  readonly statusErrors?: Readonly<Record<number, string>>;
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
    const timeout = setTimeout(() => controller.abort(), options.timeoutMs ?? 5_000);

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
        const statusError = options.statusErrors?.[response.status];
        return failure(statusError ?? readError(body, options.defaultError));
      }
      return success({ body });
    } catch {
      return failure("Couldn't reach the Axum API. Check the Base URL and try again.");
    } finally {
      clearTimeout(timeout);
    }
  }

  /** POST one immediate container lifecycle action, keeping the extended docker timeout. */
  async function containerAction(
    name: string,
    action: "start" | "stop" | "restart",
  ): Promise<Result<void>> {
    const response = await request(`/api/docker/${encodeURIComponent(name)}/${action}`, {
      method: "POST",
      // Stop/restart require a JSON body; a 5s grace keeps the server's stop
      // wrapper (grace + 5s) inside this timeout. Starts and restarts may
      // exceed it server-side and then surface as a reach error; retrying is safe.
      body: action === "start" ? undefined : { timeout_seconds: 5 },
      defaultError: `Couldn't ${action} the container.`,
      timeoutMs: DOCKER_TIMEOUT_MS,
      statusErrors: {
        404: `Container ${name} not found.`,
        503: "Docker unavailable.",
      },
    });
    return response.ok ? success(undefined) : response;
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

    docker: {
      listContainers: async (): Promise<Result<readonly DockerContainer[]>> => {
        const response = await request("/api/docker", {
          defaultError: "Couldn't load Docker containers from the Axum API.",
          timeoutMs: DOCKER_TIMEOUT_MS,
        });
        if (!response.ok) {
          return response;
        }
        try {
          return parseContainerList(JSON.parse(response.value.body));
        } catch {
          return failure(DOCKER_READ_ERROR);
        }
      },

      startContainer: (name) => containerAction(name, "start"),
      stopContainer: (name) => containerAction(name, "stop"),
      restartContainer: (name) => containerAction(name, "restart"),

      containerLogs: async (name) => {
        const response = await request(
          `/api/docker/${encodeURIComponent(name)}/logs?tail=200&timestamps=true`,
          {
            defaultError: "Couldn't load the container logs.",
            timeoutMs: DOCKER_TIMEOUT_MS,
            statusErrors: {
              404: `Container ${name} not found.`,
              503: "Docker unavailable.",
            },
          },
        );
        if (!response.ok) {
          return response;
        }
        return success(parseLogLines(response.value.body));
      },
    },

    rss: {
      compactTimeline: async (beforeId, limit): Promise<Result<readonly TimelineItem[]>> => {
        const cursor = beforeId === null ? "" : `&before_id=${beforeId}`;
        const response = await request(`/api/timeline?view=compact&limit=${limit}${cursor}`, {
          defaultError: "Couldn't load the timeline.",
        });
        if (!response.ok) {
          return response;
        }
        try {
          return parseCompactTimeline(JSON.parse(response.value.body));
        } catch {
          return failure(TIMELINE_READ_ERROR);
        }
      },
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

const DOCKER_READ_ERROR = "Couldn't read Docker containers from the Axum API.";

/** The extended request timeout shared by all docker calls. */
const DOCKER_TIMEOUT_MS = 15_000;

const TIMELINE_READ_ERROR = "Couldn't read the timeline from the Axum API.";

/**
 * Parse an untrusted compact timeline response. The compact view is a plain
 * newest-first array of feed items; any non-conforming entry rejects the payload.
 */
export function parseCompactTimeline(json: Json): Result<readonly TimelineItem[]> {
  if (!isJsonArray(json)) {
    return failure(TIMELINE_READ_ERROR);
  }
  const items: TimelineItem[] = [];
  for (const raw of json) {
    if (!isJsonObject(raw)) {
      return failure(TIMELINE_READ_ERROR);
    }
    const id = raw["id"];
    const title = raw["title"];
    const link = raw["link"];
    if (!isJsonNumber(id) || !isJsonString(title) || !isJsonString(link)) {
      return failure(TIMELINE_READ_ERROR);
    }
    const description = raw["description"];
    items.push({
      id,
      title,
      description: isJsonString(description) ? description : null,
      link,
    });
  }
  return success(items);
}

/**
 * Parse an untrusted Docker list response. The wire contract inherited from the old
 * client mixes PascalCase (`HealthStatus`, `Created`) and snake_case fields; both
 * spellings are accepted, and numeric uptime may arrive as a string.
 */
export function parseContainerList(json: Json): Result<readonly DockerContainer[]> {
  if (!isJsonObject(json) || !isJsonArray(json["containers"])) {
    return failure(DOCKER_READ_ERROR);
  }
  const containers: DockerContainer[] = [];
  for (const raw of json["containers"]) {
    if (!isJsonObject(raw)) {
      return failure(DOCKER_READ_ERROR);
    }
    const name = raw["name"];
    const state = raw["state"];
    const image = raw["image"];
    if (!isJsonString(name) || !isJsonString(state) || !isJsonString(image)) {
      return failure(DOCKER_READ_ERROR);
    }
    const health = raw["HealthStatus"] ?? raw["health_status"];
    const ports = raw["ports"];
    const restartCount = raw["restart_count"];
    containers.push({
      name,
      state,
      image,
      health: isJsonString(health) ? health : null,
      uptimeSeconds: normalizeUptime(raw["uptime_seconds"]),
      ports: isJsonArray(ports) ? ports.map(String) : [],
      restartCount: isJsonNumber(restartCount) ? restartCount : 0,
    });
  }
  return success(containers);
}

/** Normalize an uptime given as a number, a numeric string, or an unknown shape. */
function normalizeUptime(value: Json | undefined): number | null {
  if (isJsonNumber(value)) {
    return value;
  }
  if (isJsonString(value)) {
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

/** Split a plain-text logs body into lines, dropping the final newline. */
function parseLogLines(body: string): readonly string[] {
  const lines = body.split("\n");
  if (lines.length > 0 && lines[lines.length - 1] === "") {
    lines.pop();
  }
  return lines;
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
