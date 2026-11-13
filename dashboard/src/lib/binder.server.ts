import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome, readBoundedBody } from "./openhome.server"
import {
  binderEntry,
  binderList,
  cardId,
  type BinderFailure,
  type BinderState,
} from "./binder"
import {
  type BinderCommand,
  type BinderMutation,
  type BinderWriteFailure,
} from "./binder-edit"
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
    read("/api/binder/entries", binderList),
  ])

  if (owners.state !== "read") return owners

  if (entries.state !== "read") return entries

  return {
    state: "available",
    owners: owners.data,
    entries: entries.data.entries,
    values: entries.data.binder_values,
  }
}

function writeFailure(status: number): BinderWriteFailure {
  if (status === 400 || status === 422) return "invalid"

  if (status === 404) return "not-found"

  if (status === 409) return "conflict"

  return "unavailable"
}

/** Forward one owner-targeted edit or removal; only Axum's stored result confirms persistence. */
export async function writeBinder(
  command: BinderCommand
): Promise<BinderMutation> {
  const path =
    `/api/household-users/${command.owner_id}/binder/entries/${command.entry_id}` as const

  const result = await requestOpenHome(
    command.action === "edit"
      ? {
          path,
          method: "PATCH",
          json: {
            quantity: command.quantity,
            language: command.language,
            note: command.note,
          },
          timeoutMs: 5000,
        }
      : { path, method: "DELETE", timeoutMs: 5000 }
  )

  if (result.state !== "received")
    return { state: "failed", reason: result.state }

  const { response } = result

  if (!response.ok)
    return { state: "failed", reason: writeFailure(response.status) }

  if (command.action === "remove")
    return response.status === 204
      ? { state: "saved" }
      : { state: "failed", reason: "unavailable" }

  try {
    if (binderEntry.safeParse(await response.json()).success)
      return { state: "saved" }
  } catch {
    /* An invalid acknowledgement cannot confirm a saved entry. */
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
