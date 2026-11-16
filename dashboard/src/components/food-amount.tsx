import {
  formatAmount,
  supplementShare,
  type NutrientTotal,
} from "@/lib/nutrition"

/** A total's figure: a dash when unknown, "≥" when some Portions have no value. */
export function Amount({
  total,
  unit = true,
}: {
  readonly total: NutrientTotal
  readonly unit?: boolean
}): React.JSX.Element {
  if (total.value === null)
    return (
      <span className="food-amount gap-dash" data-gap="true">
        <span className="sr-only">Unknown</span>
      </span>
    )
  const partial = total.unknown_portions > 0

  return (
    <span
      className="food-amount num"
      title={
        partial
          ? `${total.unknown_portions} ${total.unknown_portions === 1 ? "portion has" : "portions have"} no value`
          : undefined
      }
    >
      {partial ? (
        <>
          <span aria-hidden="true">≥</span>
          <span className="sr-only">At least </span>
        </>
      ) : null}
      {formatAmount(total.value)}
      {unit && total.unit !== null ? (
        <span className="food-unit"> {total.unit}</span>
      ) : null}
    </span>
  )
}

/** The supplement share beneath a total's figure; nothing when no dose adds to it. */
export function SupplementShare({
  total,
  as: Element,
  className,
}: {
  readonly total: NutrientTotal
  readonly as: "p" | "dd"
  readonly className: string
}): React.JSX.Element | null {
  const share = supplementShare(total)

  return share === null ? null : (
    <Element className={className}>{share}</Element>
  )
}
