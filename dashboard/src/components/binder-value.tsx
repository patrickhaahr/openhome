import { CurrencySwitch, useCurrency } from "@/components/currency"
import type { BinderEntry, BinderValue, ValueUnavailable } from "@/lib/binder"
import { amountKey, formatMoney, type Currency } from "@/lib/currency"
import { formatDay } from "@/lib/lists"

const worth = (
  value: { usd: number; eur: number; dkk: number },
  currency: Currency
): string => formatMoney(value[amountKey[currency]], currency)

const unavailableCopy: Record<ValueUnavailable, string> = {
  japanese_printing: "No value: the catalogue prices English printings only.",
  no_catalogue_price: "No value: the catalogue has no price for this card.",
  no_exchange_rates: "No value yet: no exchange rates for the price’s day.",
}

/** The Binder Value in the chosen currency, what it leaves out, and the switch for every value, ending with the bar's actions. */
export function BinderValueBar({
  value,
  children,
}: {
  value: BinderValue | undefined
  children?: React.ReactNode
}): React.JSX.Element {
  const { currency } = useCurrency()

  return (
    <div className="lists-bar-end binder-value-bar">
      {value === undefined ? null : (
        <p className="binder-value">
          <span className="binder-value-label">Binder Value</span>
          <span className="binder-value-figure num">
            {worth(value, currency)}
          </span>
          <span className="binder-value-note">
            {value.unpriced_entries === 0
              ? null
              : `${value.unpriced_entries} unpriced. `}
            {value.oldest_observed_on === null ? (
              "No priced cards."
            ) : (
              <>
                Oldest price{" "}
                <time dateTime={value.oldest_observed_on}>
                  {formatDay(value.oldest_observed_on)}
                </time>
                .
              </>
            )}
          </span>
        </p>
      )}
      <CurrencySwitch />
      {children}
    </div>
  )
}

/** One entry's Card Value with the day it was observed, or the reason it has none. */
export function CardValueLine({
  card,
}: {
  card: BinderEntry["card"]
}): React.JSX.Element {
  const { currency } = useCurrency()
  const { card_value: value, value_unavailable: reason } = card

  return (
    <p className="binder-value">
      <span className="binder-value-label">Card Value</span>
      {value === null ? (
        <span className="binder-value-note">
          {unavailableCopy[reason ?? "no_catalogue_price"]}
        </span>
      ) : (
        <>
          <span className="binder-value-figure num">
            {worth(value, currency)}
          </span>
          <span className="binder-value-note">
            Observed{" "}
            <time dateTime={value.observed_on}>
              {formatDay(value.observed_on)}
            </time>
            {value.stale ? <span className="binder-stale">Stale</span> : null}
          </span>
        </>
      )}
    </p>
  )
}
