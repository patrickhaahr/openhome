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

const commandTimeoutMs = 12000

const discoveryTimeoutMs = 6000

const remotes = z.object({
  remotes: z.object({
    lgtv: z.array(z.string()),
    edifier: z.array(z.string()),
  }),
})

async function classifyFailure(
  response: Response
): Promise<"missing-device" | "unsupported-command" | "unavailable"> {
  const message = await readErrorMessage(response)

  if (response.status === 503 && message === "IR service is not configured") {
    return "missing-device"
  }

  if (response.status === 404 && message === "Unknown command") {
    return "unsupported-command"
  }

  return "unavailable"
}

function withButtons<C extends string>(
  options: readonly C[],
  reported: string[]
): C[] {
  const names = new Set(reported)

  return options.filter((command) => names.has(command))
}

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
    } catch {}

    return unavailableRemotes("unavailable")
  }

  const reason = await classifyFailure(response)

  return unavailableRemotes(
    reason === "unsupported-command" ? "unavailable" : reason
  )
}

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

  if (response.ok) return { state: "failed", reason: "unavailable" }

  return { state: "failed", reason: await classifyFailure(response) }
}
