import { createContext, use, useCallback, useMemo, useState } from "react"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { currencies, currencyCookie, type Currency } from "@/lib/currency"

type CurrencyState = {
  readonly currency: Currency
  readonly choose: (currency: Currency) => void
}

const CurrencyContext = createContext<CurrencyState | null>(null)

/** Holds the currency the server read from the cookie; a choice rewrites the cookie and re-renders. */
export function CurrencyProvider({
  initial,
  children,
}: {
  initial: Currency
  children: React.ReactNode
}): React.JSX.Element {
  const [currency, setCurrency] = useState(initial)

  const choose = useCallback((next: Currency) => {
    // The server reads this cookie before first paint; Cookie Store API is missing on older iPad Safari.
    // oxlint-disable-next-line unicorn/no-document-cookie -- see the comment above.
    document.cookie = currencyCookie(next)
    setCurrency(next)
  }, [])

  const state = useMemo(() => ({ currency, choose }), [currency, choose])

  return <CurrencyContext value={state}>{children}</CurrencyContext>
}

export function useCurrency(): CurrencyState {
  const state = use(CurrencyContext)

  if (state === null) throw new Error("useCurrency needs a CurrencyProvider")

  return state
}

/** One key showing the chosen currency; it opens a short list of the others. */
export function CurrencySwitch(): React.JSX.Element {
  const { currency, choose } = useCurrency()

  return (
    <Select
      value={currency}
      onValueChange={(value) => {
        const next = currencies.find((option) => option === value)

        if (next !== undefined) choose(next)
      }}
    >
      <SelectTrigger aria-label="Currency">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {currencies.map((value) => (
          <SelectItem key={value} value={value}>
            {value}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
