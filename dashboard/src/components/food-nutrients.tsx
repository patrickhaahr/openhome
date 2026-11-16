import { Amount } from "@/components/food-amount"
import { useState } from "react"
import { Button } from "@/components/ui/button"
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
        className="food-micros"
        data-state="unavailable"
        aria-labelledby="food-micros-heading"
      >
        <h2 id="food-micros-heading">No vitamin and mineral panel yet</h2>
        <p>{panel.reason}.</p>
        {panel.missing.length === 0 ? null : (
          <p className="food-micros-basis">
            Sex and birth date are set on the Workout page&apos;s Body view.
          </p>
        )}
      </section>
    )

  const { logged_days: logged } = panel.trailing_window

  return (
    <section className="food-micros" aria-labelledby="food-micros-heading">
      <h2 id="food-micros-heading">Vitamins and minerals</h2>
      <p className="food-micros-basis">
        Recommended Intakes for {referenceGroup(panel.sex, panel.age_band)} ·
        Upper Limits on {logged} logged {logged === 1 ? "day" : "days"} of the
        last 7
      </p>
      {micronutrientGroups(panel.nutrients).map((group) => (
        <section key={group.name} className="food-micro-group">
          <h3>{group.name}</h3>
          <ul className="food-micro-list">
            {group.nutrients.map((nutrient) => (
              <MicronutrientRow key={nutrient.key} nutrient={nutrient} />
            ))}
          </ul>
        </section>
      ))}
      <p className="food-micros-basis">
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

  return (
    <li
      data-key={nutrient.key}
      data-limit={status}
      data-met={share === 1 ? "" : undefined}
    >
      <span className="food-micro-name">
        {nutrient.name}
        {nutrient.adequate_intake ? (
          <span
            className="food-micro-ai"
            title="Adequate intake: a less certain reference than a recommended intake"
          >
            <span className="sr-only">, adequate intake </span>
            <span aria-hidden="true">AI</span>
          </span>
        ) : null}
      </span>
      <span className="food-micro-percent num">
        {nutrient.percent_of_recommended === null ? (
          <span data-gap="true" className="gap-dash">
            <span className="sr-only">Unknown</span>
          </span>
        ) : (
          formatPercent(nutrient.percent_of_recommended)
        )}
      </span>
      <span
        className="food-micro-bar"
        aria-hidden="true"
        // SAFETY: CSSProperties omits custom properties; --share is the reached share of the Recommended Intake.
        style={{ "--share": share ?? 0 } as React.CSSProperties}
      />
      <span className="food-micro-amount num">
        {nutrient.amount === null ? (
          <span data-gap="true" className="gap-dash">
            <span className="sr-only">Unknown</span>
          </span>
        ) : (
          formatAmount(nutrient.amount)
        )}{" "}
        / {formatAmount(nutrient.recommended_intake)}
        <span className="food-unit"> {nutrient.unit}</span>
      </span>
      {nutrient.supplements === null ? null : (
        <span className="food-micro-split">
          Food{" "}
          {nutrient.food === null
            ? "unknown"
            : `${formatAmount(nutrient.food)} ${nutrient.unit}`}{" "}
          · supplements {formatAmount(nutrient.supplements)} {nutrient.unit}
        </span>
      )}
      {note === null ? null : (
        <span className="food-micro-limit" role="note">
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
    <section className="food-nutrients" aria-label="All nutrients">
      <Button
        variant="outline"
        aria-expanded={open}
        aria-controls="food-nutrient-list"
        onClick={() => setOpen(!open)}
      >
        All nutrients
      </Button>
      {open ? (
        <div id="food-nutrient-list" className="food-nutrient-groups">
          {nutrientGroups(day.nutrients).map((group) => (
            <section key={group.name}>
              <h3>{group.name}</h3>
              <dl>
                {group.nutrients.map((nutrient) => (
                  <div key={nutrient.key}>
                    <dt>{nutrient.name_en ?? nutrient.key}</dt>
                    <dd>
                      <Amount total={nutrient} />
                    </dd>
                    {nutrient.supplements === null ? null : (
                      <dd className="food-nutrient-split">
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
