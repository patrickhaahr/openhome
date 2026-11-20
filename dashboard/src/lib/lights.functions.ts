import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { lightCommand } from "./lights"

export const sendLightCommand = createServerFn({ method: "POST" })
  .validator(lightCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { dispatchLightCommand } = await import("./lights.server")

    return dispatchLightCommand(data)
  })
