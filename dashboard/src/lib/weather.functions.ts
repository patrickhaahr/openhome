import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { readWeather } from "./weather.server"

/** Private weather read; the API key and location never enter the browser. */
export const getWeather = createServerFn({ method: "GET" }).handler(() => {
  setResponseHeader("Cache-Control", "no-store")

  return readWeather()
})
