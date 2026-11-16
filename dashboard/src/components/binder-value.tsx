import { CurrencySwitch, useCurrency } from "@/components/currency"
import { ListBarEnd } from "@/components/list-frame"
import type { BinderEntry, BinderValue, ValueUnavailable } from "@/lib/binder"
import { amountKey, formatMoney, type Currency } from "@/lib/currency"
import { formatDay } from "@/lib/lists"
import { cn } from "@/lib/utils"

const valueLabel = "text-label font-semibold text-muted-ink"

export const valueNote = "text-label text-muted-ink"

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
    <ListBarEnd
      data-slot="binder-value-bar"
      className="gap-4 binder-wide:col-start-3 binder-wide:row-start-1 binder-wide:m-0 binder-wide:flex-col binder-wide:items-end"
    >
      {value === undefined ? null : (
        <p
          data-slot="binder-value"
          className="grid min-w-60 justify-items-end gap-0.5 text-right binder-wide:min-w-0"
        >
          <span className={valueLabel}>Binder Value</span>
          <span
            data-slot="binder-value-figure"
            className="text-2xl leading-compact font-bold num"
          >
            {worth(value, currency)}
          </span>
          <span
            data-slot="binder-value-note"
            className={cn(valueNote, "binder-wide:text-balance")}
          >
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
    </ListBarEnd>
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
    <p data-slot="binder-value" className="grid gap-0.5">
      <span className={valueLabel}>Card Value</span>
      {value === null ? (
        <span data-slot="binder-value-note" className={valueNote}>
          {unavailableCopy[reason ?? "no_catalogue_price"]}
        </span>
      ) : (
        <>
          <span
            data-slot="binder-value-figure"
            className="text-binder-value leading-compact font-bold tracking-figure-tight tabular-nums"
          >
            {worth(value, currency)}
          </span>
          <span data-slot="binder-value-note" className={valueNote}>
            Observed{" "}
            <time dateTime={value.observed_on}>
              {formatDay(value.observed_on)}
            </time>
            {value.stale ? (
              <span
                data-slot="binder-stale"
                className="ml-2 inline-block rounded-full bg-danger px-2 leading-5 font-bold text-ground"
              >
                Stale
              </span>
            ) : null}
          </span>
        </>
      )}
    </p>
  )
}
