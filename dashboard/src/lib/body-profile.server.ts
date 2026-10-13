import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import {
  bodyWeight,
  profile,
  type BodyCommand,
  type BodyFailure,
  type BodyMutation,
  type BodyRange,
  type BodyState,
} from "./body-profile"

type BodyHistoryQuery = { limit: string; from?: string; to?: string }

function failure(status: number): BodyFailure {
  if (status === 409) return "duplicate-date"

  if (status === 400 || status === 422) return "invalid"

  return "unavailable"
}

async function parse<T>(
  response: Response,
  schema: z.ZodType<T>
): Promise<
  { state: "read"; data: T } | { state: "failed"; reason: BodyFailure }
> {
  if (!response.ok) return { state: "failed", reason: failure(response.status) }

  try {
    const parsed = schema.safeParse(await response.json())

    return parsed.success
      ? { state: "read", data: parsed.data }
      : { state: "failed", reason: "unavailable" }
  } catch {
    return { state: "failed", reason: "unavailable" }
  }
}

/** Read existing shared fitness data through server-held credentials, bounded to 500 history rows. */
export async function readBodyProfile(range: BodyRange): Promise<BodyState> {
  const query: BodyHistoryQuery = { limit: "500" }

  if (range.from !== undefined) query.from = range.from

  if (range.to !== undefined) query.to = range.to

  const [profileResult, historyResult] = await Promise.all([
    requestOpenHome({ path: "/api/profile", method: "GET", timeoutMs: 5000 }),
    requestOpenHome({
      path: "/api/body_weight",
      method: "GET",
      query,
      timeoutMs: 5000,
    }),
  ])

  if (profileResult.state !== "received")
    return { state: "unavailable", reason: profileResult.state }

  if (historyResult.state !== "received")
    return { state: "unavailable", reason: historyResult.state }

  const [configuration, history] = await Promise.all([
    parse(profileResult.response, profile),
    parse(historyResult.response, bodyWeight.array()),
  ])

  if (configuration.state === "failed")
    return { state: "unavailable", reason: configuration.reason }

  if (history.state === "failed")
    return { state: "unavailable", reason: history.reason }

  return {
    state: "available",
    profile: configuration.data,
    history: history.data,
  }
}

/** Forward only supported writes; Axum owns validation, null clearing and duplicate-date conflicts. */
export async function writeBodyProfile(
  command: BodyCommand
): Promise<BodyMutation> {
  const { kind, ...json } = command

  const result = await requestOpenHome({
    path: kind === "profile" ? "/api/profile" : "/api/body_weight",
    method: kind === "profile" ? "PATCH" : "POST",
    json,
    timeoutMs: 5000,
  })

  if (result.state !== "received")
    return { state: "failed", reason: result.state }

  const receipt =
    kind === "profile"
      ? await parse(result.response, profile)
      : await parse(result.response, bodyWeight)

  return receipt.state === "read" ? { state: "saved" } : receipt
}
