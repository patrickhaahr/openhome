import { failure, success, type Result } from './result';

/** A validated connection to the OpenHome Axum API. */
export type Configuration = {
  readonly baseUrl: string;
  readonly apiKey: string;
};

/** Parse setup input into a configuration safe for API requests. */
export function parseConfiguration(baseUrlInput: string, apiKeyInput: string): Result<Configuration> {
  const baseUrl = normalizeSetupInput(baseUrlInput).replace(/\/+$/, '');
  const apiKey = normalizeSetupInput(apiKeyInput);

  if (baseUrl.length === 0) {
    return failure('Enter a Base URL.');
  }
  if (apiKey.length === 0) {
    return failure('Enter an API Key.');
  }

  let url: URL;
  try {
    url = new URL(baseUrl);
  } catch {
    return failure('Base URL must include a valid host.');
  }

  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    return failure('Base URL must use http or https.');
  }
  if (url.hostname.length === 0) {
    return failure('Base URL must include a host.');
  }
  if (url.search.length > 0 || url.hash.length > 0) {
    return failure('Base URL must not include a query or fragment.');
  }

  return success({ baseUrl, apiKey });
}

function normalizeSetupInput(value: string): string {
  const trimmed = value.trim();
  if (trimmed.startsWith('"') && trimmed.endsWith('"') && trimmed.length >= 2) {
    return trimmed.slice(1, -1).trim();
  }
  return trimmed;
}
