import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"

/** Check Axum on the server; browsers receive only a sanitized outcome and timestamp. */
export const getConnectionHealth = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readConnectionHealth } = await import("./connection.server")

    return readConnectionHealth()
  }
)
