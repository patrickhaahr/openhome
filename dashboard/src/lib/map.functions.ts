import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { readMapOrigin } from "./map.server"

/** Private map origin read; the API key never enters the browser. */
export const getMapOrigin = createServerFn({ method: "GET" }).handler(() => {
  setResponseHeader("Cache-Control", "no-store")

  return readMapOrigin()
})
