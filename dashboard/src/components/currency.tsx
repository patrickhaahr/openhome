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

export function CurrencyProvider({
  initial,
  children,
}: {
  initial: Currency
  children: React.ReactNode
}): React.JSX.Element {
  const [currency, setCurrency] = useState(initial)

  const choose = useCallback((next: Currency) => {
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
