export type NavLayout = "top" | "side"

export type NavLabels = "words" | "icons" | "both"

export type NavPrefs = {
  readonly layout: NavLayout
  readonly labels: NavLabels
}

export const navCookieName = "openhome-nav"

export const navLabelsCookieName = "openhome-nav-labels"

const oneYear = 60 * 60 * 24 * 365

export const doubleTapWindow = 350

export function parseNavLayout(value: string | undefined): NavLayout {
  return value === "side" ? "side" : "top"
}

export function parseNavLabels(value: string | undefined): NavLabels {
  return value === "icons" || value === "both" ? value : "words"
}

export function cookieValue(cookies: string, name: string): string | undefined {
  const entry = cookies
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(`${name}=`))

  return entry?.slice(name.length + 1)
}

export function navLayoutFromCookies(cookies: string): NavLayout {
  return parseNavLayout(cookieValue(cookies, navCookieName))
}

export function navLabelsFromCookies(cookies: string): NavLabels {
  return parseNavLabels(cookieValue(cookies, navLabelsCookieName))
}

export const rememberedCookie = (name: string, value: string): string =>
  `${name}=${value}; Path=/; Max-Age=${oneYear}; SameSite=Lax`

export function navLayoutCookie(layout: NavLayout): string {
  return rememberedCookie(navCookieName, layout)
}

export function navLabelsCookie(labels: NavLabels): string {
  return rememberedCookie(navLabelsCookieName, labels)
}

export function otherLayout(layout: NavLayout): NavLayout {
  return layout === "top" ? "side" : "top"
}

export function isDoubleTap(
  previousAt: number | undefined,
  now: number
): boolean {
  return previousAt !== undefined && now - previousAt <= doubleTapWindow
}
