import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import type { BinderState } from "./binder"
import { binderCommand } from "./binder-edit"

/** Household-visible read through authenticated Axum, with credentials held by Start. */
export const getBinder = createServerFn({ method: "GET" }).handler(async () => {
  setResponseHeader("Cache-Control", "no-store")
  const { readBinder } = await import("./binder.server")

  return readBinder()
})

/** Explicit-owner edits and removals use the same Axum rules as MCP. */
export const mutateBinder = createServerFn({ method: "POST" })
  .validator(binderCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeBinder } = await import("./binder.server")

    return writeBinder(data)
  })

/** A dropped RPC keeps recovery reachable within the focused view. */
export async function loadBinder(): Promise<BinderState> {
  try {
    return await getBinder()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
