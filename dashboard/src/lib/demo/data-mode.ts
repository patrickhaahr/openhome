import { cookieValue, rememberedCookie } from "../nav-layout"

export type DataMode = "live" | "demo"

export const dataModeCookieName = "openhome-data"

export const dataModeSearchParam = "demo"

export function parseDataMode(value: string | undefined): DataMode {
  return value === "demo" ? "demo" : "live"
}

export function dataModeFromCookies(cookies: string): DataMode {
  return parseDataMode(cookieValue(cookies, dataModeCookieName))
}

export function dataModeCookie(mode: DataMode): string {
  return rememberedCookie(dataModeCookieName, mode)
}

export function otherDataMode(mode: DataMode): DataMode {
  return mode === "demo" ? "live" : "demo"
}

export type DataModeSwitch = {
  readonly mode: DataMode
  readonly location: string
}

export function dataModeSwitch(url: URL): DataModeSwitch | undefined {
  const requested = url.searchParams.get(dataModeSearchParam)

  if (requested !== "on" && requested !== "off") return undefined

  const rest = new URL(url)

  rest.searchParams.delete(dataModeSearchParam)

  return {
    mode: requested === "on" ? "demo" : "live",
    location: `${rest.pathname}${rest.search}`,
  }
}
