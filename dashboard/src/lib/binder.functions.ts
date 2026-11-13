import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import type { BinderState } from "./binder"

/** Household-visible read through authenticated Axum, with credentials held by Start. */
export const getBinder = createServerFn({ method: "GET" }).handler(async () => {
  setResponseHeader("Cache-Control", "no-store")
  const { readBinder } = await import("./binder.server")

  return readBinder()
})

/** A dropped RPC keeps recovery reachable within the focused view. */
export async function loadBinder(): Promise<BinderState> {
  try {
    return await getBinder()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
