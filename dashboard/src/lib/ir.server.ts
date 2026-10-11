import "@tanstack/react-start/server-only"
import { z } from "zod"
import { type IrCommandResult, type IrRemotes, unavailableRemotes } from "./ir"
import {
  isAcknowledged,
  readErrorMessage,
  requestOpenHome,
} from "./openhome.server"
import { speakerCommand, type SpeakerCommand } from "./speaker"
import { tvCommand, type TvCommand } from "./tv"

// Axum's IR device request has a 10-second timeout. Allow a command to return its outcome.
const commandTimeoutMs = 12000

// Discovery only fills the remotes; a stalled bridge should not hold their cards on "Checking…".
const discoveryTimeoutMs = 6000

const remotes = z.object({
  remotes: z.object({
    lgtv: z.array(z.string()),
    edifier: z.array(z.string()),
  }),
})

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

/** Keep the reported Commands that have Remote Buttons, in remote order. */
function withButtons<C extends string>(
  options: readonly C[],
  reported: string[]
): C[] {
  const names = new Set(reported)

  return options.filter((command) => names.has(command))
}

/** Ask Axum which Commands each IR Remote reports, keeping only those with Remote Buttons. */
export async function readIrRemotes(): Promise<IrRemotes> {
  const result = await requestOpenHome({
    path: "/api/ir",
    method: "GET",
    timeoutMs: discoveryTimeoutMs,
  })

  if (result.state !== "received") return unavailableRemotes(result.state)

  const { response } = result

  if (response.ok) {
    try {
      const parsed = remotes.safeParse(await response.json())

      if (parsed.success) {
        const { lgtv, edifier } = parsed.data.remotes

        return {
          tv: {
            state: "available",
            commands: withButtons(tvCommand.options, lgtv),
          },
          speaker: {
            state: "available",
            commands: withButtons(speakerCommand.options, edifier),
          },
        }
      }
    } catch {
      // An unreadable response cannot establish which commands are available.
    }

    return unavailableRemotes("unavailable")
  }

  const reason = await classifyFailure(response)

  // A stale-command answer cannot apply to discovery.
  return unavailableRemotes(
    reason === "unsupported-command" ? "unavailable" : reason
  )
}

/**
 * Dispatch one Command through Axum. Provider messages never become observed device state or public copy.
 *
 * @param options.remote - Axum's path segment for the IR Remote that sends the Command.
 * @param options.command - A Command parsed against that remote's Remote Buttons.
 * @returns Whether OpenHome acknowledged the Command, with a sanitized failure reason.
 */
export async function dispatchIrCommand({
  remote,
  command,
}:
  | { remote: "lgtv"; command: TvCommand }
  | { remote: "edifier"; command: SpeakerCommand }): Promise<IrCommandResult> {
  const result = await requestOpenHome({
    path: `/api/ir/${remote}`,
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
