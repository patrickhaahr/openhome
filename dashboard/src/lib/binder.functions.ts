import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { z } from "zod"
import { cardId, type BinderState } from "./binder"
import type { PricesState } from "./card-prices"
import { binderCommand } from "./binder-edit"
import { binderAddition, cardSearchInput } from "./binder-add"

export const getBinder = createServerFn({ method: "GET" }).handler(async () => {
  setResponseHeader("Cache-Control", "no-store")
  const { readBinder } = await import("./binder.server")

  return readBinder()
})

export const mutateBinder = createServerFn({ method: "POST" })
  .validator(binderCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeBinder } = await import("./binder.server")

    return writeBinder(data)
  })

export async function loadBinder(): Promise<BinderState> {
  try {
    return await getBinder()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}

export const searchCards = createServerFn({ method: "GET" })
  .validator(cardSearchInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { searchBinderCards } = await import("./binder.server")

    return searchBinderCards(data)
  })

export const addCard = createServerFn({ method: "POST" })
  .validator(binderAddition)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { addBinderEntry } = await import("./binder.server")

    return addBinderEntry(data)
  })

export const getCardPrices = createServerFn({ method: "GET" })
  .validator(z.object({ card: cardId }))
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readCardPrices } = await import("./binder.server")

    return readCardPrices(data)
  })

export async function loadCardPrices(card: string): Promise<PricesState> {
  try {
    return await getCardPrices({ data: { card } })
  } catch {
    return { state: "unavailable" }
  }
}
