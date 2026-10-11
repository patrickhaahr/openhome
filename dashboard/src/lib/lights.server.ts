import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import type { LightCommand, LightCommandResult } from "./lights"

const acknowledgement = z.object({ message: z.string() })

const missingSetup = z.object({
  error: z.literal("SwitchBot service is not configured"),
})

/** Dispatch once through Axum. Provider messages never become observed state or public copy. */
export async function dispatchLightCommand(
  command: LightCommand
): Promise<LightCommandResult> {
  const result = await requestOpenHome({
    path: `/api/lights/${command}`,
    method: "POST",
    // Axum's SwitchBot request has a 10-second timeout. Allow it to return its outcome.
    timeoutMs: 12000,
  })

  if (result.state !== "received") {
    return { state: "failed", reason: result.state }
  }

  const { response } = result

  try {
    const body: unknown = await response.json()

    if (response.ok && acknowledgement.safeParse(body).success) {
      return { state: "accepted" }
    }

    if (response.status === 503 && missingSetup.safeParse(body).success) {
      return { state: "failed", reason: "missing-device" }
    }
  } catch {
    // An unreadable acknowledgement cannot establish whether a command was accepted.
  }

  return { state: "failed", reason: "unavailable" }
}
