import { cn } from "@/lib/utils"
import {
  formatAmount,
  supplementShare,
  type NutrientTotal,
} from "@/lib/nutrition"

/** The heading over a group of nutrients. */
export const groupHeading = "mb-2 text-small font-semibold text-muted-ink"

/** The capsule that marks an adequate intake, a less certain reference. */
export const adequateIntake =
  "rounded-full bg-wash-strong px-2 py-px text-caption font-semibold text-muted-ink"

/** A unit after its figure: smaller, muted, and outside the figure's tracking. */
export function Unit({
  children,
}: {
  readonly children: React.ReactNode
}): React.JSX.Element {
  return (
    <span className="text-food-unit tracking-normal text-muted-ink">
      {children}
    </span>
  )
}

/** A source line under a page. */
export function Credit({
  children,
}: {
  readonly children: React.ReactNode
}): React.JSX.Element {
  return (
    <p data-slot="food-credit" className="text-label text-muted-ink">
      {children}
    </p>
  )
}

/** A total's figure: a dash when unknown, "≥" when some Portions have no value. */
export function Amount({
  total,
  unit = true,
  className,
}: {
  readonly total: NutrientTotal
  readonly unit?: boolean
  readonly className?: string
}): React.JSX.Element {
  if (total.value === null)
    return (
      <span
        data-slot="food-amount"
        data-gap="true"
        className={cn("data-[gap=true]:gap-dash", className)}
      >
        <span className="sr-only">Unknown</span>
      </span>
    )
  const partial = total.unknown_portions > 0

  return (
    <span
      data-slot="food-amount"
      className={cn("num", className)}
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
      {unit && total.unit !== null ? <Unit> {total.unit}</Unit> : null}
    </span>
  )
}

/** The supplement share beneath a total's figure; nothing when no dose adds to it. */
export function SupplementShare({
  total,
  as: Element,
  className,
  slot,
}: {
  readonly total: NutrientTotal
  readonly as: "p" | "dd"
  readonly className: string
  readonly slot?: string
}): React.JSX.Element | null {
  const share = supplementShare(total)

  return share === null ? null : (
    <Element data-slot={slot} className={className}>
      {share}
    </Element>
  )
}
