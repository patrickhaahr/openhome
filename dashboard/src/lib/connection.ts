export type ConnectionState =
  | "connected"
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"

export type ConnectionHealth = {
  state: ConnectionState
  checkedAt: string
}
