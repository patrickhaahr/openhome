import { cookieValue, rememberedCookie } from "./nav-layout"

export const currencies = ["DKK", "EUR", "USD"] as const

export type Currency = (typeof currencies)[number]

export const currencyCookieName = "openhome-currency"

export function parseCurrency(value: string | undefined): Currency {
  return value === "EUR" || value === "USD" ? value : "DKK"
}

export function currencyFromCookies(cookies: string): Currency {
  return parseCurrency(cookieValue(cookies, currencyCookieName))
}

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
