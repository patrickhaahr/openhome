/** Where the navigation sits from 641 px up; a phone always uses the bottom tab bar. */
export type NavLayout = "top" | "side"

/** How the top bar names its tabs: words, glyphs, or a glyph beside each word. */
export type NavLabels = "words" | "icons" | "both"

/** Both navigation choices, as the server read them before first paint. */
export type NavPrefs = {
  readonly layout: NavLayout
  readonly labels: NavLabels
}

export const navCookieName = "openhome-nav"

export const navLabelsCookieName = "openhome-nav-labels"

const oneYear = 60 * 60 * 24 * 365

/** Two clicks on the logo inside this window switch the layout. */
export const doubleTapWindow = 350

/** Anything but an explicit "side" is the top bar. */
export function parseNavLayout(value: string | undefined): NavLayout {
  return value === "side" ? "side" : "top"
}

/** Anything but an explicit "icons" or "both" is words. */
export function parseNavLabels(value: string | undefined): NavLabels {
  return value === "icons" || value === "both" ? value : "words"
}

/** One cookie's value from a `Cookie` header or `document.cookie`. */
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

const rememberedCookie = (name: string, value: string): string =>
  `${name}=${value}; Path=/; Max-Age=${oneYear}; SameSite=Lax`

/** The `document.cookie` assignment that remembers a layout for a year. */
export function navLayoutCookie(layout: NavLayout): string {
  return rememberedCookie(navCookieName, layout)
}

/** The `document.cookie` assignment that remembers the top bar's labels for a year. */
export function navLabelsCookie(labels: NavLabels): string {
  return rememberedCookie(navLabelsCookieName, labels)
}

export function otherLayout(layout: NavLayout): NavLayout {
  return layout === "top" ? "side" : "top"
}

/** A click counts as the second of a double tap when the first landed within the window. */
export function isDoubleTap(
  previousAt: number | undefined,
  now: number
): boolean {
  return previousAt !== undefined && now - previousAt <= doubleTapWindow
}
