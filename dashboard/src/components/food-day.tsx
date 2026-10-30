import { useState } from "react"
import { Link } from "@tanstack/react-router"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { ForkKnifeIcon } from "@phosphor-icons/react/dist/csr/ForkKnife"
import { Button } from "@/components/ui/button"
import {
  dayName,
  energySplit,
  foodDatabaseCredit,
  formatAmount,
  mealTime,
  nutrientGroups,
  nutritionFailureMessage,
  shiftDay,
  type MealLog,
  type NutrientTotal,
  type NutritionDay,
  type NutritionDayState,
} from "@/lib/nutrition"

const macros = [
  ["protein", "Protein"],
  ["available_carbohydrate", "Carbs"],
  ["fat", "Fat"],
  ["dietary_fibre", "Fibre"],
  ["alcohol", "Alcohol"],
] as const

const splitNames = {
  protein: "Protein",
  available_carbohydrate: "Carbs",
  fat: "Fat",
  alcohol: "Alcohol",
} as const

/**
 * The Food page's Day view: one Copenhagen day's Meal Logs in time order with the day's energy
 * and macronutrient totals, every nutrient on request, and the Food database's credit.
 */
export function FoodDay({
  data,
}: {
  readonly data: NutritionDayState
}): React.JSX.Element {
  return (
    <div className="food">
      <DayNav day={data.day} today={data.today} />
      <DayBody data={data} />
      <p className="food-credit">
        Nutrient values{" "}
        {data.state === "available"
          ? `${data.data.source.credit} (${data.data.source.licence})`
          : `${foodDatabaseCredit} (CC BY 4.0)`}
      </p>
    </div>
  )
}

function DayBody({
  data,
}: {
  readonly data: NutritionDayState
}): React.JSX.Element {
  switch (data.state) {
    case "available":
      return <Day day={data.data} />
    case "food-database-unavailable":
      return (
        <div className="state-note food-unavailable" role="status">
          <h2>
            {data.database === "importing"
              ? "Food database importing"
              : "Food database unavailable"}
          </h2>
          <p>
            {data.database === "importing"
              ? "The Danish Food Composition Database is being imported. This day shows once it is ready."
              : "The food database could not be imported. OpenHome tries again when it restarts."}
          </p>
        </div>
      )
    case "unavailable":
      return (
        <div className="state-note food-unavailable" role="status">
          <h2>Food unavailable</h2>
          <p>{nutritionFailureMessage[data.reason]}</p>
        </div>
      )
  }
}

/** Previous and next keys around the day's name; there is no next day after today. */
function DayNav({
  day,
  today,
}: {
  readonly day: string
  readonly today: string
}): React.JSX.Element {
  const previous = shiftDay(day, -1)
  const next = shiftDay(day, 1)

  // Today is the page's default, so it keeps a clean URL.
  const searchFor = (target: string) =>
    target === today ? {} : { day: target }

  return (
    <nav className="food-days" aria-label="Day">
      <Link
        to="/food"
        search={searchFor(previous)}
        className="food-day-key"
        aria-label={`Previous day, ${dayName(previous, today)}`}
      >
        <CaretLeftIcon size={22} weight="bold" aria-hidden="true" />
      </Link>
      <h2 className="food-day-name" aria-live="polite">
        {dayName(day, today)}
      </h2>
      {day < today ? (
        <Link
          to="/food"
          search={searchFor(next)}
          className="food-day-key"
          aria-label={`Next day, ${dayName(next, today)}`}
        >
          <CaretRightIcon size={22} weight="bold" aria-hidden="true" />
        </Link>
      ) : (
        <span className="food-day-key" aria-hidden="true" data-disabled="">
          <CaretRightIcon size={22} weight="bold" />
        </span>
      )}
      {day === today ? null : (
        <Link to="/food" search={{}} className="food-today">
          Today
        </Link>
      )}
    </nav>
  )
}

/** A total's figure: a dash when unknown, "≥" when some Portions have no value. */
function Amount({
  total,
  unit = true,
}: {
  readonly total: NutrientTotal
  readonly unit?: boolean
}): React.JSX.Element {
  if (total.value === null)
    return (
      <span className="food-amount" data-gap="true">
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

function Day({ day }: { readonly day: NutritionDay }): React.JSX.Element {
  const split = energySplit(day.totals)

  return (
    <>
      <section className="food-totals" aria-label="Day totals">
        <p className="food-energy">
          <Amount total={day.totals.energy_kcal} unit={false} />
          <span className="food-energy-unit">kcal</span>
        </p>
        {split.length === 0 ? null : (
          <div
            className="food-split"
            role="img"
            aria-label={`Energy from ${split
              .filter((part) => part.kcal > 0)
              .map(
                (part) =>
                  `${splitNames[part.key].toLowerCase()} ${Math.round(part.share * 100)} %`
              )
              .join(", ")}`}
          >
            {split.map((part) =>
              part.kcal > 0 ? (
                <span
                  key={part.key}
                  data-macro={part.key}
                  // SAFETY: CSSProperties omits custom properties; --share is the macro's energy share.
                  style={{ "--share": part.share } as React.CSSProperties}
                />
              ) : null
            )}
          </div>
        )}
        <dl className="food-macros">
          {macros.map(([key, name]) => (
            <div key={key} data-macro={key}>
              <dt>{name}</dt>
              <dd>
                <Amount total={day.totals[key]} />
              </dd>
            </div>
          ))}
        </dl>
      </section>
      <section aria-labelledby="food-meals-heading">
        <h2 id="food-meals-heading" className="sr-only">
          Meals
        </h2>
        {day.meal_logs.length === 0 ? (
          <div className="food-empty">
            <ForkKnifeIcon size={40} aria-hidden="true" />
            <h3>Nothing logged</h3>
            <p className="sr-only">
              Meals logged through the agent appear here at the time they were
              eaten.
            </p>
          </div>
        ) : (
          <ol className="food-meals" aria-label="Meals in time order">
            {day.meal_logs.map((meal) => (
              <li key={meal.id}>
                <Meal meal={meal} />
              </li>
            ))}
          </ol>
        )}
      </section>
      <AllNutrients day={day} />
    </>
  )
}

function Meal({ meal }: { readonly meal: MealLog }): React.JSX.Element {
  return (
    <article className="food-meal">
      <header>
        <h3>
          <time dateTime={meal.eaten_at}>{mealTime(meal.eaten_at)}</time>
        </h3>
        <Amount total={meal.totals.energy_kcal} />
      </header>
      {meal.note === null ? null : <p className="food-note">{meal.note}</p>}
      <ul className="food-portions" aria-label="Portions">
        {meal.portions.map((portion, index) => (
          <li key={`${portion.food_id}-${index}`}>
            <span className="food-portion-name">{portion.name_en}</span>
            <span className="food-portion-grams num">
              {formatAmount(portion.grams)} g
            </span>
            <span className="food-portion-kcal num">
              {portion.nutrients.energy_kcal.value === null ? (
                <span data-gap="true">
                  <span className="sr-only">Energy unknown</span>
                </span>
              ) : (
                `${formatAmount(portion.nutrients.energy_kcal.value)} kcal`
              )}
            </span>
          </li>
        ))}
      </ul>
    </article>
  )
}

/** Every parameter's total, grouped as the Food database groups them, behind one key. */
function AllNutrients({
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
