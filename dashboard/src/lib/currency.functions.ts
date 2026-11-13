import { createServerOnlyFn } from "@tanstack/react-start"
import { getCookie } from "@tanstack/react-start/server"
import {
  currencyCookieName,
  currencyFromCookies,
  parseCurrency,
  type Currency,
} from "./currency"

const readRequestCurrency = createServerOnlyFn((): Currency =>
  parseCurrency(getCookie(currencyCookieName))
)

/** The remembered currency: the request cookie on the server, so the first paint is right. */
export function readCurrency(): Currency {
  return typeof document === "undefined"
    ? readRequestCurrency()
    : currencyFromCookies(document.cookie)
}
