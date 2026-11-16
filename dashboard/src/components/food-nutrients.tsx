import {
  adequateIntake,
  Amount,
  groupHeading,
  Unit,
} from "@/components/food-amount"
import { useState } from "react"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import {
  formatAmount,
  formatPercent,
  intakeShare,
  micronutrientGroups,
  nutrientGroups,
  referenceGroup,
  upperLimitNote,
  type Micronutrient,
  type Micronutrients,
  type NutritionDay,
} from "@/lib/nutrition"

const basis = "text-small text-muted-ink"

/** The amount and its supplement split sit under the bar across both columns. */
const reference = "col-span-full text-label text-muted-ink"

/** The bar's fill by what the row shows: an Upper Limit first, then a reached intake. */
function barFill(status: string | undefined, met: boolean): string {
  if (status === "cut_down") return "before:bg-danger"

  if (status === "caution") return "before:bg-food-caution"

  return met ? "before:bg-ink" : "before:bg-muted-ink"
}

/**
 * The vitamins and minerals as percentages of their NNR 2023 Recommended Intakes, with adequate
 * intakes marked, the supplement share noted and EFSA Upper Limits as a caution (the day above
 * one) or a red "cut down" (the 7-day average above one); or why there is no panel.
 */
export function MicronutrientPanel({
  panel,
}: {
  readonly panel: Micronutrients
}): React.JSX.Element {
  if (panel.state === "unavailable")
    return (
      <section
        data-slot="food-micros"
        className="grid gap-4"
        data-state="unavailable"
        aria-labelledby="food-micros-heading"
      >
        <h2
          id="food-micros-heading"
          className="text-lead font-medium tracking-food-heading"
        >
          No vitamin and mineral panel yet
        </h2>
        <p>{panel.reason}.</p>
        {panel.missing.length === 0 ? null : (
          <p data-slot="food-micros-basis" className={basis}>
            Sex and birth date are set on the Workout page&apos;s Body view.
          </p>
        )}
      </section>
    )

  const { logged_days: logged } = panel.trailing_window

  return (
    <section
      data-slot="food-micros"
      className="grid gap-4"
      aria-labelledby="food-micros-heading"
    >
      <h2
        id="food-micros-heading"
        className="text-lead font-medium tracking-food-heading"
      >
        Vitamins and minerals
      </h2>
      <p data-slot="food-micros-basis" className={basis}>
        Recommended Intakes for {referenceGroup(panel.sex, panel.age_band)} ·
        Upper Limits on {logged} logged {logged === 1 ? "day" : "days"} of the
        last 7
      </p>
      {micronutrientGroups(panel.nutrients).map((group, index) => (
        <section
          key={group.name}
          data-slot="food-micro-group"
          className={cn(index > 0 && "mt-2")}
        >
          <h3 className={groupHeading}>{group.name}</h3>
          <ul
            data-slot="food-micro-list"
            className="grid grid-fill-60 gap-x-8 gap-y-4"
          >
            {group.nutrients.map((nutrient) => (
              <MicronutrientRow key={nutrient.key} nutrient={nutrient} />
            ))}
          </ul>
        </section>
      ))}
      <p data-slot="food-micros-basis" className={basis}>
        {panel.sources.map((source) => source.citation).join(" · ")}
      </p>
    </section>
  )
}

/** A nutrient's percentage over a bar, its amount and reference, and any Upper Limit line. */
function MicronutrientRow({
  nutrient,
}: {
  readonly nutrient: Micronutrient
}): React.JSX.Element {
  const share = intakeShare(nutrient)
  const note = upperLimitNote(nutrient)

  const status =
    nutrient.upper_limit_status === "none"
      ? undefined
      : (nutrient.upper_limit_status ?? undefined)

  const cutDown = status === "cut_down"

  return (
    <li
      data-key={nutrient.key}
      data-limit={status}
      className="grid grid-cols-trail items-baseline gap-x-3 gap-y-1"
    >
      <span
        data-slot="food-micro-name"
        className="flex items-baseline gap-2 font-semibold"
      >
        {nutrient.name}
        {nutrient.adequate_intake ? (
          <span
            className={adequateIntake}
            title="Adequate intake: a less certain reference than a recommended intake"
          >
            <span className="sr-only">, adequate intake </span>
            <span aria-hidden="true">AI</span>
          </span>
        ) : null}
      </span>
      <span
        data-slot="food-micro-percent"
        className={cn("text-large font-medium num", cutDown && "text-danger")}
      >
        {nutrient.percent_of_recommended === null ? (
          <span data-gap="true" className="data-[gap=true]:gap-dash">
            <span className="sr-only">Unknown</span>
          </span>
        ) : (
          formatPercent(nutrient.percent_of_recommended)
        )}
      </span>
      <span
        className={cn(
          "col-span-full h-1.5 overflow-hidden rounded-full bg-wash-strong before:block before:h-full before:w-share before:rounded-full",
          barFill(status, share === 1)
        )}
        aria-hidden="true"
        // SAFETY: CSSProperties omits custom properties; --share is the reached share of the Recommended Intake.
        style={{ "--share": share ?? 0 } as React.CSSProperties}
      />
      <span data-slot="food-micro-amount" className={cn("num", reference)}>
        {nutrient.amount === null ? (
          <span data-gap="true" className="data-[gap=true]:gap-dash">
            <span className="sr-only">Unknown</span>
          </span>
        ) : (
          formatAmount(nutrient.amount)
        )}{" "}
        / {formatAmount(nutrient.recommended_intake)}
        <Unit> {nutrient.unit}</Unit>
      </span>
      {nutrient.supplements === null ? null : (
        <span data-slot="food-micro-split" className={reference}>
          Food{" "}
          {nutrient.food === null
            ? "unknown"
            : `${formatAmount(nutrient.food)} ${nutrient.unit}`}{" "}
          · supplements {formatAmount(nutrient.supplements)} {nutrient.unit}
        </span>
      )}
      {note === null ? null : (
        <span
          data-slot="food-micro-limit"
          className={cn(
            "col-span-full flex items-baseline gap-2 text-label font-semibold before:size-2 before:flex-none before:rounded-full",
            cutDown
              ? "text-danger before:bg-danger"
              : "text-ink before:bg-food-caution"
          )}
          role="note"
        >
          {note}
        </span>
      )}
    </li>
  )
}

/** Every parameter's total, grouped as the Food database groups them, behind one key. */
export function AllNutrients({
  day,
}: {
  readonly day: NutritionDay
}): React.JSX.Element {
  const [open, setOpen] = useState(false)

  return (
    <section
      className="grid justify-items-start gap-5"
      aria-label="All nutrients"
    >
      <Button
        variant="outline"
        aria-expanded={open}
        aria-controls="food-nutrient-list"
        onClick={() => setOpen(!open)}
      >
        All nutrients
      </Button>
      {open ? (
        <div
          id="food-nutrient-list"
          data-slot="food-nutrient-groups"
          className="grid w-full grid-fit-68 gap-x-10 gap-y-6"
        >
          {nutrientGroups(day.nutrients).map((group) => (
            <section key={group.name}>
              <h3 className={groupHeading}>{group.name}</h3>
              <dl>
                {group.nutrients.map((nutrient) => (
                  <div
                    key={nutrient.key}
                    className="flex flex-wrap justify-between gap-4 py-stack odd:-mx-2 odd:rounded-food-row odd:bg-wash odd:px-2"
                  >
                    <dt>{nutrient.name_en ?? nutrient.key}</dt>
                    <dd>
                      <Amount total={nutrient} />
                    </dd>
                    {nutrient.supplements === null ? null : (
                      <dd className="basis-full text-label text-muted-ink">
                        Food{" "}
                        {nutrient.food === null
                          ? "unknown"
                          : `${formatAmount(nutrient.food)} ${nutrient.unit ?? ""}`.trim()}{" "}
                        · supplements{" "}
                        {`${formatAmount(nutrient.supplements)} ${nutrient.unit ?? ""}`.trim()}
                      </dd>
                    )}
                  </div>
                ))}
              </dl>
            </section>
          ))}
        </div>
      ) : null}
    </section>
  )
}
