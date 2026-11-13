import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { z } from "zod"
import { cardId, type BinderState } from "./binder"
import type { PricesState } from "./card-prices"
import { binderCommand } from "./binder-edit"
import { binderAddition, cardSearchInput } from "./binder-add"

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

/** A card's Price History, read when its sheet opens. */
export const getCardPrices = createServerFn({ method: "GET" })
  .validator(z.object({ card: cardId }))
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readCardPrices } = await import("./binder.server")

    return readCardPrices(data)
  })

/** A dropped RPC reads as unavailable, never as an empty history. */
export async function loadCardPrices(card: string): Promise<PricesState> {
  try {
    return await getCardPrices({ data: { card } })
  } catch {
    return { state: "unavailable" }
  }
}
