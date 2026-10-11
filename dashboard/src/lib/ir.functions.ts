import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import type { IrRemotes } from "./ir.server"
import { speakerCommand } from "./speaker"
import { tvCommand } from "./tv"

/** Discover both IR Remotes on the server; browsers receive only known command names. */
export const getIrRemotes = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readIrRemotes } = await import("./ir.server")

    return readIrRemotes()
  }
)

/** Send one TV Command through Axum on the server; browsers receive only a sanitized outcome. */
export const sendTvCommand = createServerFn({ method: "POST" })
  .validator(tvCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { dispatchIrCommand } = await import("./ir.server")

    return dispatchIrCommand({ remote: "lgtv", command: data })
  })

/** Send one speaker Command through Axum on the server; browsers receive only a sanitized outcome. */
export const sendSpeakerCommand = createServerFn({ method: "POST" })
  .validator(speakerCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { dispatchIrCommand } = await import("./ir.server")

    return dispatchIrCommand({ remote: "edifier", command: data })
  })

/**
 * Discover IR Remotes for a route loader without letting a dropped RPC reject the page.
 *
 * @returns The discovered remotes, or an unreachable outcome when the dashboard server did not answer.
 */
export async function loadIrRemotes(): Promise<IrRemotes> {
  try {
    return await getIrRemotes()
  } catch {
    const unreachable = { state: "unavailable", reason: "unreachable" } as const

    return { tv: unreachable, speaker: unreachable }
  }
}
