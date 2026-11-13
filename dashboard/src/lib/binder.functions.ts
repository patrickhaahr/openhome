import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import type { BinderState } from "./binder"
import { binderAddition, cardSearchInput } from "./binder-add"

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

/** A deliberate Card Code lookup; Start holds the credential. */
export const searchCards = createServerFn({ method: "GET" })
  .validator(cardSearchInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { searchBinderCards } = await import("./binder.server")

    return searchBinderCards(data)
  })

/** An explicit-owner addition; a retry carries the same `request_id`. */
export const addCard = createServerFn({ method: "POST" })
  .validator(binderAddition)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { addBinderEntry } = await import("./binder.server")

    return addBinderEntry(data)
  })
