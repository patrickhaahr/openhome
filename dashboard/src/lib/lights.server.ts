import "@tanstack/react-start/server-only"
import {
  isAcknowledged,
  readErrorMessage,
  requestOpenHome,
} from "./openhome.server"
import type { LightCommand, LightCommandResult } from "./lights"

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

  if (await isAcknowledged(response)) return { state: "accepted" }

  if (
    response.status === 503 &&
    (await readErrorMessage(response)) === "SwitchBot service is not configured"
  ) {
    return { state: "failed", reason: "missing-device" }
  }

  return { state: "failed", reason: "unavailable" }
}
