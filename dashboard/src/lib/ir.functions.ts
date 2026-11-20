import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { type IrRemotes, unavailableRemotes } from "./ir"
import { speakerCommand } from "./speaker"
import { tvCommand } from "./tv"

export const getIrRemotes = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readIrRemotes } = await import("./ir.server")

    return readIrRemotes()
  }
)

export const sendTvCommand = createServerFn({ method: "POST" })
  .validator(tvCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { dispatchIrCommand } = await import("./ir.server")

    return dispatchIrCommand({ remote: "lgtv", command: data })
  })

export const sendSpeakerCommand = createServerFn({ method: "POST" })
  .validator(speakerCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { dispatchIrCommand } = await import("./ir.server")

    return dispatchIrCommand({ remote: "edifier", command: data })
  })

export async function loadIrRemotes(): Promise<IrRemotes> {
  try {
    return await getIrRemotes()
  } catch {
    return unavailableRemotes("unreachable")
  }
}
