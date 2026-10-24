import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { backlightAction } from "./backlight"
import { readBacklight, changeBacklight } from "./backlight.server"

/** Read sanitized status without caching; accepted requests remain distinct from observations. */
export const getBacklight = createServerFn({ method: "GET" }).handler(() => {
  setResponseHeader("Cache-Control", "no-store")

  return readBacklight()
})

/** Validate an action and return its sanitized API outcome without caching or private details. */
export const updateBacklight = createServerFn({ method: "POST" })
  .validator(backlightAction)
  .handler(({ data }) => {
    setResponseHeader("Cache-Control", "no-store")

    return changeBacklight(data)
  })
