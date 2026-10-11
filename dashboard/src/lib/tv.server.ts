import "@tanstack/react-start/server-only"
import { z } from "zod"
import {
  isAcknowledged,
  readErrorMessage,
  requestOpenHome,
} from "./openhome.server"
import {
  tvCommand,
  type TvCommand,
  type TvCommandResult,
  type TvRemoteState,
} from "./tv"

// Axum's IR device request has a 10-second timeout. Allow a command to return its outcome.
const commandTimeoutMs = 12000

// Discovery only fills the remote; a stalled bridge should not hold its card on "Checking…".
const discoveryTimeoutMs = 6000

const remotes = z.object({ remotes: z.object({ lgtv: z.array(z.string()) }) })

/** Classify an unsuccessful Axum response; only exact known errors earn specific guidance. */
async function classifyFailure(
  response: Response
): Promise<"missing-device" | "unsupported-command" | "unavailable"> {
  const message = await readErrorMessage(response)

  if (response.status === 503 && message === "IR service is not configured") {
    return "missing-device"
  }

  // The IR remote rejects commands it does not know; discovery may be stale.
  if (response.status === 404 && message === "Unknown command") {
    return "unsupported-command"
  }

  return "unavailable"
}

/** Ask Axum which TV commands the IR remote reports, keeping only those with Remote Buttons. */
export async function readTvRemote(): Promise<TvRemoteState> {
  const result = await requestOpenHome({
    path: "/api/ir",
    method: "GET",
    timeoutMs: discoveryTimeoutMs,
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

  const reason = await classifyFailure(response)

  // A stale-command answer cannot apply to discovery.
  return {
    state: "unavailable",
    reason: reason === "unsupported-command" ? "unavailable" : reason,
  }
}

/** Dispatch once through Axum. Provider messages never become observed TV state or public copy. */
export async function dispatchTvCommand(
  command: TvCommand
): Promise<TvCommandResult> {
  const result = await requestOpenHome({
    path: "/api/ir/lgtv",
    method: "POST",
    timeoutMs: commandTimeoutMs,
    json: { command },
  })

  if (result.state !== "received") {
    return { state: "failed", reason: result.state }
  }

  const { response } = result

  if (await isAcknowledged(response)) return { state: "accepted" }

  // A successful status without an acknowledgement cannot confirm the command.
  if (response.ok) return { state: "failed", reason: "unavailable" }

  return { state: "failed", reason: await classifyFailure(response) }
}
