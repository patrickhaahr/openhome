import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { readWeather } from "./weather.server"

export const getWeather = createServerFn({ method: "GET" }).handler(() => {
  setResponseHeader("Cache-Control", "no-store")

  return readWeather()
})
