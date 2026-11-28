import { createServerOnlyFn } from "@tanstack/react-start"
import { getCookie } from "@tanstack/react-start/server"
import {
  dataModeCookieName,
  dataModeFromCookies,
  parseDataMode,
  type DataMode,
} from "./data-mode"

const readRequestDataMode = createServerOnlyFn((): DataMode =>
  parseDataMode(getCookie(dataModeCookieName))
)

export function readDataMode(): DataMode {
  return typeof document === "undefined"
    ? readRequestDataMode()
    : dataModeFromCookies(document.cookie)
}
