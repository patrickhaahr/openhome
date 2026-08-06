/** The result of an operation that can fail in an expected way. */
export type Result<T> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly error: string };

/** Create a successful result. */
export function success<T>(value: T): Result<T> {
  return { ok: true, value };
}

/** Create a failed result. */
export function failure<T = never>(error: string): Result<T> {
  return { ok: false, error };
}
