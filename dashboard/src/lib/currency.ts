import { cookieValue, rememberedCookie } from "./nav-layout"

/** The three currencies a Card Value and a Binder Value are shown in. */
export const currencies = ["DKK", "EUR", "USD"] as const

export type Currency = (typeof currencies)[number]

export const currencyCookieName = "openhome-currency"

/** Anything but an explicit "EUR" or "USD" is DKK, the household's currency. */
export function parseCurrency(value: string | undefined): Currency {
  return value === "EUR" || value === "USD" ? value : "DKK"
}

export function currencyFromCookies(cookies: string): Currency {
  return parseCurrency(cookieValue(cookies, currencyCookieName))
}

/** The `document.cookie` assignment that remembers a currency for a year. */
export function currencyCookie(currency: Currency): string {
  return rememberedCookie(currencyCookieName, currency)
}

export const amountKey = { DKK: "dkk", EUR: "eur", USD: "usd" } as const

const formats: Record<Currency, Intl.NumberFormat> = {
  DKK: new Intl.NumberFormat("en-GB", { style: "currency", currency: "DKK" }),
  EUR: new Intl.NumberFormat("en-GB", { style: "currency", currency: "EUR" }),
  USD: new Intl.NumberFormat("en-GB", { style: "currency", currency: "USD" }),
}

export function formatMoney(amount: number, currency: Currency): string {
  return formats[currency].format(amount)
}
