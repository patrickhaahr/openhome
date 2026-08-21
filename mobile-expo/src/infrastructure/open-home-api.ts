import type { Configuration } from "../domain/configuration";
import { isJsonArray, isJsonObject, isJsonString, type Json } from "../domain/json";
import { failure, success, type Result } from "../domain/result";

/** Status and available command sets returned by the IR API. */
export type IrStatus = {
  readonly message: string;
  readonly edifierCommands: ReadonlySet<string>;
  readonly lgTvCommands: ReadonlySet<string>;
};

/** Operations used by the OpenHome application layer. */
export type OpenHomeApi = {
  readonly validateConfiguration: () => Promise<Result<void>>;
  readonly getIrStatus: () => Promise<Result<IrStatus>>;
  readonly sendIrCommand: (remote: "edifier" | "lgtv", command: string) => Promise<Result<void>>;
  readonly sendLightCommand: (command: "on" | "off") => Promise<Result<void>>;
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
