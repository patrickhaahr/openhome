import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  bodyCommand,
  bodyRange,
  type BodyRange,
  type BodyState,
} from "./body-profile"

/** Read the singleton Profile and daily Body Weight through authenticated Axum. */
export const getBodyProfile = createServerFn({ method: "GET" })
  .validator(bodyRange)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readBodyProfile } = await import("./body-profile.server")

    return readBodyProfile(data)
  })

/** Persist a Profile patch or Body Weight entry, with the credential kept on the server. */
export const mutateBodyProfile = createServerFn({ method: "POST" })
  .validator(bodyCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeBodyProfile } = await import("./body-profile.server")

    return writeBodyProfile(data)
  })

/** A dropped read RPC offers recovery controls rather than an empty history. */
export async function loadBodyProfile(range: BodyRange): Promise<BodyState> {
  try {
    return await getBodyProfile({ data: range })
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
