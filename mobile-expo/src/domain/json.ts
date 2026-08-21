/** Data decodable from JSON text at an I/O boundary. */
export type Json =
  | string
  | number
  | boolean
  | null
  | readonly Json[]
  | { readonly [key: string]: Json };

/** Decide whether a JSON value is a JSON object. */
export function isJsonObject(value: Json | undefined): value is { readonly [key: string]: Json } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Decide whether a JSON value is a JSON array. */
export function isJsonArray(value: Json | undefined): value is readonly Json[] {
  return Array.isArray(value);
}

/** Decide whether a JSON value is a string. */
export function isJsonString(value: Json | undefined): value is string {
  return typeof value === "string";
}

/** Decide whether a JSON value is a finite number. */
export function isJsonNumber(value: Json | undefined): value is number {
  return Number.isFinite(value);
}
