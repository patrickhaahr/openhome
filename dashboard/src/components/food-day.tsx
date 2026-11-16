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
import { Amount, Credit, SupplementShare } from "@/components/food-amount"
import { StateNote, stateNoteText } from "@/components/page-tools"

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
    <div data-slot="food" className="grid max-w-240 gap-food-flow">
      <FoodViews view="day" />
      <DayNav day={data.day} today={data.today} />
      {data.state === "available" ? (
        <>
          <TargetStrip targets={data.data.targets} />
          <GoalControl goal={data.goal} save={saveGoal} reload={reload} />
        </>
      ) : null}
      <DayBody data={data} />
      <Credit>
        Nutrient values{" "}
        {data.state === "available"
          ? `${data.data.source.credit} (${data.data.source.licence})`
          : `${foodDatabaseCredit} (CC BY 4.0)`}
      </Credit>
      {data.state === "available" && hasProducts(data.data) ? (
        <Credit>
          {`${data.data.label_source.credit} (${data.data.label_source.licence})`}
        </Credit>
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
        <StateNote
          data-slot="food-unavailable"
          role="status"
          title={
            data.database === "importing"
              ? "Food database importing"
              : "Food database unavailable"
          }
        >
          <p className={stateNoteText}>
            {data.database === "importing"
              ? "The Danish Food Composition Database is being imported. This day shows once it is ready."
              : "The food database could not be imported. OpenHome tries again when it restarts."}
          </p>
        </StateNote>
      )
    case "unavailable":
      return (
        <StateNote
          data-slot="food-unavailable"
          role="status"
          title="Food unavailable"
        >
          <p className={stateNoteText}>
            {nutritionFailureMessage[data.reason]}
          </p>
        </StateNote>
      )
  }
}

const dayKey =
  "inline-grid size-12 place-items-center rounded-full bg-wash-strong text-ink transition-food-key duration-fast ease-app"

/** The reversed look of a key under the pointer, shared by the day keys and "Today". */
const keyHover = "hover:bg-ink hover:text-ground"

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
    <nav className="flex flex-wrap items-center gap-3" aria-label="Day">
      <Link
        to="/food"
        search={searchFor(previous)}
        data-slot="food-day-key"
        className={`${dayKey} ${keyHover} active:scale-97`}
        aria-label={`Previous day, ${dayName(previous, today)}`}
      >
        <CaretLeftIcon size={22} weight="bold" aria-hidden="true" />
      </Link>
      <h2
        data-slot="food-day-name"
        className="min-w-food-day text-center text-lead font-medium tracking-food-heading"
        aria-live="polite"
      >
        {dayName(day, today)}
      </h2>
      {day < today ? (
        <Link
          to="/food"
          search={searchFor(next)}
          data-slot="food-day-key"
          className={`${dayKey} ${keyHover} active:scale-97`}
          aria-label={`Next day, ${dayName(next, today)}`}
        >
          <CaretRightIcon size={22} weight="bold" aria-hidden="true" />
        </Link>
      ) : (
        <span
          data-slot="food-day-key"
          className={`${dayKey} data-disabled:opacity-35`}
          aria-hidden="true"
          data-disabled=""
        >
          <CaretRightIcon size={22} weight="bold" />
        </span>
      )}
      {day === today ? null : (
        <Link
          to="/food"
          search={{}}
          data-slot="food-today"
          className={`ml-auto inline-flex min-h-11 items-center rounded-full bg-wash px-4.5 font-semibold text-ink ${keyHover}`}
        >
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
      <section className="grid gap-4" aria-label="Day totals">
        <p
          data-slot="food-energy"
          className="flex items-baseline gap-2 text-display leading-food-energy font-medium"
        >
          <Amount total={day.totals.energy_kcal} unit={false} />
          <span className="text-lead tracking-figure-tight text-muted-ink">
            kcal
          </span>
        </p>
        <SupplementShare
          total={day.totals.energy_kcal}
          as="p"
          slot="food-energy-supplements"
          className="-mt-2 text-label text-muted-ink"
        />
        {split.length === 0 ? null : (
          <div
            data-slot="food-split"
            className="flex h-2.5 gap-0.75 overflow-hidden rounded-full bg-wash"
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
                  className="min-w-0.75 flex-(--share) bg-macro"
                  // SAFETY: CSSProperties omits custom properties; --share is the macro's energy share.
                  style={{ "--share": part.share } as React.CSSProperties}
                />
              ) : null
            )}
          </div>
        )}
        <dl
          data-slot="food-macros"
          className="grid grid-fit-30 gap-x-6 gap-y-3"
        >
          {macros.map(([key, name]) => (
            <div key={key} data-macro={key}>
              <dt
                className={`flex items-center gap-2 text-control text-muted-ink before:size-2.5 before:rounded-full before:bg-macro ${key === "dietary_fibre" ? "before:inset-ring-2 before:inset-ring-muted-ink" : ""}`}
              >
                {name}
              </dt>
              <dd className="text-lead font-medium">
                <Amount total={day.totals[key]} />
              </dd>
              <SupplementShare
                total={day.totals[key]}
                as="dd"
                className="text-label text-muted-ink"
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
          <div
            data-slot="food-empty"
            className="grid justify-items-start gap-3 py-10"
          >
            <ForkKnifeIcon
              className="text-muted-ink"
              size={40}
              aria-hidden="true"
            />
            <h3 className="text-title leading-none font-medium tracking-headline">
              Nothing logged
            </h3>
            <p className="sr-only">
              Meals logged through the agent appear here at the time they were
              eaten.
            </p>
          </div>
        ) : (
          <ol
            data-slot="food-meals"
            className="grid gap-3"
            aria-label="Meals in time order"
          >
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

const supplement =
  "inline-flex items-baseline gap-2 rounded-full bg-wash-strong px-3 py-1 text-small"

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
      data-slot="food-supplements"
      className="grid gap-2"
      aria-labelledby="food-supplements-heading"
    >
      <h3
        id="food-supplements-heading"
        className="text-control font-medium text-muted-ink"
      >
        Supplements
      </h3>
      <ul className="flex flex-wrap gap-2">
        {day.supplement_doses.map((dose) => (
          <li key={dose.supplement_id} className={supplement}>
            <span className="font-semibold">{dose.name}</span>
            <span className="num text-muted-ink">
              {doseCount(dose.doses, dose.dose_unit)}
            </span>
          </li>
        ))}
        {day.compounds.map((compound) => (
          <li key={compound.name} className={supplement}>
            <span className="font-semibold">{compound.name}</span>
            <span className="num text-muted-ink">
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
    <div data-slot="food-nova" className="grid gap-1">
      <p className="flex items-baseline gap-2 text-lead font-medium">
        {nova.nova_1_share === null ? (
          <span
            data-slot="food-amount"
            data-gap="true"
            className="data-[gap=true]:gap-dash"
          >
            <span className="sr-only">Unknown</span>
          </span>
        ) : (
          <span data-slot="food-amount" className="num">
            {Math.round(nova.nova_1_share * 100)} %
          </span>
        )}{" "}
        <span className="text-control text-muted-ink">NOVA 1</span>
      </p>
      <p className="text-small text-muted-ink">{novaDetail(nova)}</p>
    </div>
  )
}
