import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  bodyCommand,
  bodyRange,
  type BodyRange,
  type BodyState,
} from "./body-profile"

export const getBodyProfile = createServerFn({ method: "GET" })
  .validator(bodyRange)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readBodyProfile } = await import("./body-profile.server")

    return readBodyProfile(data)
  })

export const mutateBodyProfile = createServerFn({ method: "POST" })
  .validator(bodyCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeBodyProfile } = await import("./body-profile.server")

    return writeBodyProfile(data)
  })

export async function loadBodyProfile(range: BodyRange): Promise<BodyState> {
  try {
    return await getBodyProfile({ data: range })
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
