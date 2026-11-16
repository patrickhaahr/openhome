import { Link } from "@tanstack/react-router"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { ForkKnifeIcon } from "@phosphor-icons/react/dist/csr/ForkKnife"
import { FoodViews } from "@/components/food-views"
import {
  dayName,
  doseCount,
  energySplit,
  foodDatabaseCredit,
  formatAmount,
  hasProducts,
  novaDetail,
  nutritionFailureMessage,
  shiftDay,
  type GoalMutation,
  type NutritionDay,
  type NutritionDayState,
  type SetGoalInput,
} from "@/lib/nutrition"
import { TargetStrip, GoalControl } from "@/components/food-targets"
import { Meal } from "@/components/food-meal"
import { MicronutrientPanel, AllNutrients } from "@/components/food-nutrients"
import { Amount, SupplementShare } from "@/components/food-amount"

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

type FoodDayProps = {
  readonly data: NutritionDayState
  /** Set the Nutrition Goal from today: the Day view's only write. */
  readonly saveGoal: (goal: SetGoalInput) => Promise<GoalMutation>
  /** Read the day again after the goal changed. */
  readonly reload: () => Promise<void>
}

/**
 * The Food page's Day view: one Copenhagen day's Calorie Target, eaten and remaining with the
 * macros against their targets and the goal control, then its Meal Logs in time order with the
 * day's energy and macronutrient totals and their supplement share, the Supplements taken, the
 * vitamins and minerals against their Recommended Intakes and Upper Limits, every nutrient on
 * request, and the Food database's credit.
 */
export function FoodDay({
  data,
  saveGoal,
  reload,
}: FoodDayProps): React.JSX.Element {
  return (
    <div className="food">
      <FoodViews view="day" />
      <DayNav day={data.day} today={data.today} />
      {data.state === "available" ? (
        <>
          <TargetStrip targets={data.data.targets} />
          <GoalControl goal={data.goal} save={saveGoal} reload={reload} />
        </>
      ) : null}
      <DayBody data={data} />
      <p className="food-credit">
        Nutrient values{" "}
        {data.state === "available"
          ? `${data.data.source.credit} (${data.data.source.licence})`
          : `${foodDatabaseCredit} (CC BY 4.0)`}
      </p>
      {data.state === "available" && hasProducts(data.data) ? (
        <p className="food-credit">
          {`${data.data.label_source.credit} (${data.data.label_source.licence})`}
        </p>
      ) : null}
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

function Day({ day }: { readonly day: NutritionDay }): React.JSX.Element {
  const split = energySplit(day.totals)

  return (
    <>
      <section className="food-totals" aria-label="Day totals">
        <p className="food-energy">
          <Amount total={day.totals.energy_kcal} unit={false} />
          <span className="food-energy-unit">kcal</span>
        </p>
        <SupplementShare
          total={day.totals.energy_kcal}
          as="p"
          className="food-energy-supplements"
        />
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
              <SupplementShare
                total={day.totals[key]}
                as="dd"
                className="food-macro-supplements"
              />
            </div>
          ))}
        </dl>
        <NovaShare nova={day.nova} />
        <Supplements day={day} />
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
      <MicronutrientPanel panel={day.micronutrients} />
      <AllNutrients day={day} />
    </>
  )
}

/**
 * The day's Supplements: each Supplement's doses added together, then the compounds without a
 * database parameter as amounts taken. Nothing when no Supplement was taken.
 */
function Supplements({
  day,
}: {
  readonly day: NutritionDay
}): React.JSX.Element | null {
  if (day.supplement_doses.length === 0 && day.compounds.length === 0)
    return null

  return (
    <section
      className="food-supplements"
      aria-labelledby="food-supplements-heading"
    >
      <h3 id="food-supplements-heading">Supplements</h3>
      <ul>
        {day.supplement_doses.map((dose) => (
          <li key={dose.supplement_id}>
            <span className="food-supplement-name">{dose.name}</span>
            <span className="food-supplement-amount num">
              {doseCount(dose.doses, dose.dose_unit)}
            </span>
          </li>
        ))}
        {day.compounds.map((compound) => (
          <li key={compound.name} data-compound="">
            <span className="food-supplement-name">{compound.name}</span>
            <span className="food-supplement-amount num">
              {formatAmount(compound.value)} {compound.unit}
            </span>
          </li>
        ))}
      </ul>
    </section>
  )
}

/** The share of the day's classified energy from NOVA Class 1, with what it leaves out. */
function NovaShare({
  nova,
}: {
  readonly nova: NutritionDay["nova"]
}): React.JSX.Element {
  return (
    <div className="food-nova">
      <p className="food-nova-share">
        {nova.nova_1_share === null ? (
          <span className="food-amount gap-dash" data-gap="true">
            <span className="sr-only">Unknown</span>
          </span>
        ) : (
          <span className="food-amount num">
            {Math.round(nova.nova_1_share * 100)} %
          </span>
        )}{" "}
        <span className="food-nova-name">NOVA 1</span>
      </p>
      <p className="food-nova-detail">{novaDetail(nova)}</p>
    </div>
  )
}
