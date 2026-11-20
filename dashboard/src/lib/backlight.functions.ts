import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { backlightAction } from "./backlight"
import { readBacklight, changeBacklight } from "./backlight.server"

export const getBacklight = createServerFn({ method: "GET" }).handler(() => {
  setResponseHeader("Cache-Control", "no-store")

  return readBacklight()
})

export const updateBacklight = createServerFn({ method: "POST" })
  .validator(backlightAction)
  .handler(({ data }) => {
    setResponseHeader("Cache-Control", "no-store")

    return changeBacklight(data)
  })
