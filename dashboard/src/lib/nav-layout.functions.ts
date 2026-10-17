import { createServerOnlyFn } from "@tanstack/react-start"
import { getCookie } from "@tanstack/react-start/server"
import {
  navCookieName,
  navLabelsCookieName,
  navLabelsFromCookies,
  navLayoutFromCookies,
  parseNavLabels,
  parseNavLayout,
  type NavPrefs,
} from "./nav-layout"

const readRequestNavPrefs = createServerOnlyFn((): NavPrefs => ({
  layout: parseNavLayout(getCookie(navCookieName)),
  labels: parseNavLabels(getCookie(navLabelsCookieName)),
}))

/** The remembered navigation: the request cookies on the server, so the first paint is right. */
export function readNavPrefs(): NavPrefs {
  return typeof document === "undefined"
    ? readRequestNavPrefs()
    : {
        layout: navLayoutFromCookies(document.cookie),
        labels: navLabelsFromCookies(document.cookie),
      }
}
