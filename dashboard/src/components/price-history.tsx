import { useEffect, useState } from "react"
import { useCurrency } from "@/components/currency"
import { TrendChart } from "@/components/trend-chart"
import { formatMoney } from "@/lib/currency"
import { priceLine, priceSummary, type PricesState } from "@/lib/card-prices"

/**
 * One Card's Price History as a line in the chosen currency, on the Trends chart: a day with no
 * observation is a gap in the line, never a flat stretch. The history is read when the sheet opens.
 */
export function PriceHistory({
  card,
  load,
}: {
  card: string
  load: (card: string) => Promise<PricesState>
}): React.JSX.Element {
  const { currency } = useCurrency()
  const [prices, setPrices] = useState<PricesState>({ state: "loading" })

  useEffect(() => {
    let current = true

    void load(card).then((next) => {
      if (current) setPrices(next)
    })

    return () => {
      current = false
    }
  }, [card, load])

  if (prices.state === "loading")
    return (
      <p role="status" className="binder-value-note">
        Loading price history…
      </p>
    )

  if (prices.state === "unavailable")
    return (
      <p role="status" className="binder-value-note">
        Price history is unavailable. Check again in a moment.
      </p>
    )

  if (prices.points.length === 0)
    return (
      <p className="binder-value-note">
        No price has been recorded for this card yet.
      </p>
    )

  const line = priceLine(prices.points, currency)

  return (
    <TrendChart
      chart="card-price"
      title="Price History"
      summary={priceSummary(line, currency)}
      labels={line.labels}
      names={line.names}
      partial={line.labels.map(() => false)}
      marks={[
        {
          kind: "line",
          key: "price",
          name: `Price in ${currency}`,
          tone: "ink",
          values: line.values,
          gap: "No observation",
        },
      ]}
      format={(value) => formatMoney(value, currency)}
    />
  )
}
