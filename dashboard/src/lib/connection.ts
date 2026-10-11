/** Sanitized outcomes; private runtime values never enter this contract. */
export type ConnectionState =
  | "connected"
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"

/** The result and time of one authenticated health check, safe to serialize. */
export type ConnectionHealth = {
  state: ConnectionState
  checkedAt: string
}
