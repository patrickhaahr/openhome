import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { lightCommand } from "./lights"

/** Send one Light Command through Axum on the server; browsers receive only a sanitized outcome. */
export const sendLightCommand = createServerFn({ method: "POST" })
  .validator(lightCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { dispatchLightCommand } = await import("./lights.server")

    return dispatchLightCommand(data)
  })
