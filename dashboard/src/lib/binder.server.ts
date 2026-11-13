import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome, readBoundedBody } from "./openhome.server"
import {
  binderEntry,
  cardId,
  type BinderFailure,
  type BinderState,
} from "./binder"
import {
  cardSearchResponse,
  type AddFailure,
  type BinderAddReceipt,
  type BinderAddition,
  type CardSearchResult,
} from "./binder-add"
import { householdUser } from "./wishlist"

async function read<T>(
  path: `/api/${string}`,
  schema: z.ZodType<T>
): Promise<
  { state: "read"; data: T } | { state: "unavailable"; reason: BinderFailure }
> {
  const result = await requestOpenHome({ path, method: "GET", timeoutMs: 5000 })

  if (result.state !== "received")
    return { state: "unavailable", reason: result.state }

  if (!result.response.ok)
    return { state: "unavailable", reason: "unavailable" }

  try {
    const parsed = schema.safeParse(await result.response.json())

    if (parsed.success) return { state: "read", data: parsed.data }
  } catch {
    /* Only safe protocol values are read. */
  }

  return { state: "unavailable", reason: "unavailable" }
}

/** Read owners and every Binder in parallel through the server-held credential. */
export async function readBinder(): Promise<BinderState> {
  const [owners, entries] = await Promise.all([
    read("/api/household-users", householdUser.array()),
    read("/api/binder/entries", binderEntry.array()),
  ])

  if (owners.state !== "read") return owners

  if (entries.state !== "read") return entries

  return { state: "available", owners: owners.data, entries: entries.data }
}

/** Search by Card Code through Start's server boundary; the catalogue is never reached by the browser. */
export async function searchBinderCards(input: {
  card_code: string
}): Promise<CardSearchResult> {
  const result = await requestOpenHome({
    path: "/api/binder/cards",
    method: "GET",
    query: input,
    timeoutMs: 20000,
  })

  if (result.state !== "received") return { state: "failed" }
  const { response } = result

  if (response.status === 400) return { state: "invalid" }

  if (response.status === 503) return { state: "outage" }

  if (!response.ok) return { state: "failed" }

  try {
    const parsed = cardSearchResponse.safeParse(await response.json())

    if (parsed.success)
      return parsed.data.cards.length === 0
        ? { state: "unlisted" }
        : { state: "found", cards: parsed.data.cards }
  } catch {
    /* Never expose private catalogue diagnostics. */
  }

  return { state: "failed" }
}

function additionFailure(status: number): AddFailure {
  if (status === 400 || status === 422) return "invalid"

  if (status === 404) return "not-found"

  if (status === 409) return "conflict"

  if (status === 503) return "catalogue"

  return "unavailable"
}

const addition = z.object({ entry: binderEntry, replayed: z.boolean() })

/** Forward one explicit-owner addition; only a parsed receipt confirms that it was stored. */
export async function addBinderEntry(
  command: BinderAddition
): Promise<BinderAddReceipt> {
  const result = await requestOpenHome({
    path: "/api/binder/entries",
    method: "POST",
    json: command,
    timeoutMs: 15000,
  })

  if (result.state !== "received")
    return { state: "failed", reason: result.state }

  if (!result.response.ok)
    return { state: "failed", reason: additionFailure(result.response.status) }

  try {
    if (addition.safeParse(await result.response.json()).success)
      return { state: "saved" }
  } catch {
    /* An invalid acknowledgement cannot confirm a saved addition. */
  }

  return { state: "failed", reason: "unavailable" }
}

const imageLimit = 2 * 1024 * 1024

/** Relay one stored AVIF as bounded bytes, never upstream headers or error bodies. */
export async function readCardImage(input: {
  card: string
}): Promise<Response> {
  const headers = {
    "X-Content-Type-Options": "nosniff",
    "Content-Security-Policy":
      "default-src 'none'; style-src 'unsafe-inline'; sandbox",
  }

  const failed = (status: number): Response =>
    new Response(null, {
      status,
      headers: { ...headers, "Cache-Control": "no-store" },
    })

  const parsed = cardId.safeParse(input.card)

  if (!parsed.success) return failed(404)

  const result = await requestOpenHome({
    path: `/api/binder/cards/${parsed.data}/image`,
    method: "GET",
    timeoutMs: 12000,
  })

  if (result.state !== "received") return failed(503)
  const { response } = result

  if (!response.ok || response.headers.get("content-type") !== "image/avif") {
    await response.body?.cancel()

    return failed(response.status === 404 ? 404 : 503)
  }

  const body = await readBoundedBody(response, imageLimit)

  if (body === undefined) return failed(503)

  return new Response(body, {
    headers: {
      ...headers,
      "Cache-Control": "private, max-age=86400",
      "Content-Type": "image/avif",
    },
  })
}
