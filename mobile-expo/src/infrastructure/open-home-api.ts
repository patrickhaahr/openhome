import type { Configuration } from '../domain/configuration';
import { failure, success, type Result } from '../domain/result';

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
  readonly sendIrCommand: (remote: 'edifier' | 'lgtv', command: string) => Promise<Result<void>>;
  readonly sendLightCommand: (command: 'on' | 'off') => Promise<Result<void>>;
};

type RequestOptions = {
  readonly method?: 'GET' | 'POST';
  readonly body?: object;
  readonly defaultError: string;
};

type ApiResponse = { readonly body: string };

/** Create an HTTP adapter scoped to one validated configuration. */
export function createOpenHomeApi(configuration: Configuration): OpenHomeApi {
  async function request(path: string, options: RequestOptions): Promise<Result<ApiResponse>> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 5_000);

    try {
      const response = await fetch(`${configuration.baseUrl}${path}`, {
        method: options.method ?? 'GET',
        headers: {
          Authorization: `Bearer ${configuration.apiKey}`,
          ...(options.body === undefined ? {} : { 'Content-Type': 'application/json' }),
        },
        body: options.body === undefined ? undefined : JSON.stringify(options.body),
        redirect: 'manual',
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
      const response = await request('/api/health', {
        defaultError: 'OpenHome rejected that Base URL or API Key.',
      });
      return response.ok ? success(undefined) : response;
    },

    async getIrStatus(): Promise<Result<IrStatus>> {
      const response = await request('/api/ir', {
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

    async sendIrCommand(remote: 'edifier' | 'lgtv', command: string): Promise<Result<void>> {
      const response = await request(`/api/ir/${remote}`, {
        method: 'POST',
        body: { command },
        defaultError: "Couldn't send that IR command to the Axum API.",
      });
      return response.ok ? success(undefined) : response;
    },

    async sendLightCommand(command: 'on' | 'off'): Promise<Result<void>> {
      const response = await request(`/api/lights/${command}`, {
        method: 'POST',
        defaultError: "Couldn't switch the light.",
      });
      return response.ok ? success(undefined) : response;
    },
  };
}

/** Parse an untrusted IR status response. */
export function parseIrStatus(value: unknown): Result<IrStatus> {
  if (typeof value !== 'object' || value === null || !('remotes' in value)) {
    return failure("Couldn't read IR status from the Axum API.");
  }
  const remotes = value.remotes;
  if (typeof remotes !== 'object' || remotes === null || !('edifier' in remotes) || !('lgtv' in remotes)) {
    return failure("Couldn't read IR status from the Axum API.");
  }
  if (!isStringArray(remotes.edifier) || !isStringArray(remotes.lgtv)) {
    return failure("Couldn't read IR status from the Axum API.");
  }

  const message = 'message' in value && typeof value.message === 'string' && value.message.trim().length > 0
    ? value.message.trim()
    : 'IR remote ready';
  return success({
    message,
    edifierCommands: new Set(remotes.edifier.map((command) => command.trim()).filter(Boolean)),
    lgTvCommands: new Set(remotes.lgtv.map((command) => command.trim()).filter(Boolean)),
  });
}

function readError(body: string, fallback: string): string {
  try {
    const value: unknown = JSON.parse(body);
    if (typeof value === 'object' && value !== null && 'error' in value && typeof value.error === 'string' && value.error.trim().length > 0) {
      return value.error.trim();
    }
  } catch {
    return fallback;
  }
  return fallback;
}

function isStringArray(value: unknown): value is ReadonlyArray<string> {
  return Array.isArray(value) && value.every((item) => typeof item === 'string');
}
