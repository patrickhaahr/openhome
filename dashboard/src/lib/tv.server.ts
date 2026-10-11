import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import {
  tvCommand,
  type TvCommand,
  type TvCommandResult,
  type TvRemoteState,
} from "./tv"

// Axum's IR device request has a 10-second timeout. Allow it to return its outcome.
const irTimeoutMs = 12000

const acknowledgement = z.object({ message: z.string() })

const remotes = z.object({ remotes: z.object({ lgtv: z.array(z.string()) }) })

const missingSetup = z.object({
  error: z.literal("IR service is not configured"),
})

async function isMissingSetup(response: Response): Promise<boolean> {
  if (response.status !== 503) return false

  try {
    return missingSetup.safeParse(await response.json()).success
  } catch {
    return false
  }
}

/** Ask Axum which TV commands the IR remote reports, keeping only those with Remote Buttons. */
export async function readTvRemote(): Promise<TvRemoteState> {
  const result = await requestOpenHome({
    path: "/api/ir",
    method: "GET",
    timeoutMs: irTimeoutMs,
  })

  if (result.state !== "received") {
    return { state: "unavailable", reason: result.state }
  }

  const { response } = result

  if (response.ok) {
    try {
      const parsed = remotes.safeParse(await response.json())

      if (parsed.success) {
        const reported = new Set(parsed.data.remotes.lgtv)

        return {
          state: "available",
          commands: tvCommand.options.filter((command) =>
            reported.has(command)
          ),
        }
      }
    } catch {
      // An unreadable response cannot establish which commands are available.
    }

    return { state: "unavailable", reason: "unavailable" }
  }

  return {
    state: "unavailable",
    reason: (await isMissingSetup(response)) ? "missing-device" : "unavailable",
  }
}

/** Dispatch once through Axum. Provider messages never become observed TV state or public copy. */
export async function dispatchTvCommand(
  command: TvCommand
): Promise<TvCommandResult> {
  const result = await requestOpenHome({
    path: "/api/ir/lgtv",
    method: "POST",
    timeoutMs: irTimeoutMs,
    json: { command },
  })

  if (result.state !== "received") {
    return { state: "failed", reason: result.state }
  }

  const { response } = result

  if (response.ok) {
    try {
      if (acknowledgement.safeParse(await response.json()).success) {
        return { state: "accepted" }
      }
    } catch {
      // An unreadable acknowledgement cannot establish whether a command was accepted.
    }

    return { state: "failed", reason: "unavailable" }
  }

  // The IR remote rejects commands it does not know; discovery may be stale.
  if (response.status === 404) {
    return { state: "failed", reason: "unsupported-command" }
  }

  return {
    state: "failed",
    reason: (await isMissingSetup(response)) ? "missing-device" : "unavailable",
  }
}
