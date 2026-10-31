import { useId, useRef, useState } from "react"
import { Link } from "@tanstack/react-router"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { ForkKnifeIcon } from "@phosphor-icons/react/dist/csr/ForkKnife"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  dayName,
  doseCount,
  goalChoice,
  goalChoices,
  goalName,
  doseEnergy,
  energySplit,
  foodDatabaseCredit,
  formatAmount,
  hasProducts,
  mealPhotoUrl,
  mealTime,
  nutrientGroups,
  novaClassMeaning,
  novaDetail,
  nutritionFailureMessage,
  portionName,
  shiftDay,
  supplementShare,
  type DayTargets,
  type GoalMutation,
  type MealLog,
  type MealPhoto,
  type NutrientTotal,
  type NutritionDay,
  type NutritionDayState,
  type NutritionGoal,
  type Portion,
  type SetGoalInput,
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
 * day's energy and macronutrient totals and their supplement share, the Supplements taken, every
 * nutrient on request, and the Food database's credit.
 */
export function FoodDay({
  data,
  saveGoal,
  reload,
}: FoodDayProps): React.JSX.Element {
  return (
    <div className="food">
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

/** The supplement share beneath a total's figure; nothing when no dose adds to it. */
function SupplementShare({
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
          <span className="food-amount" data-gap="true">
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

function Meal({ meal }: { readonly meal: MealLog }): React.JSX.Element {
  return (
    <article
      className="food-meal"
      data-photo={meal.photo === null ? undefined : ""}
    >
      <div className="food-meal-body">
        <MealDetails meal={meal} />
      </div>
      {meal.photo === null ? null : (
        <MealPhotoKey photo={meal.photo} time={mealTime(meal.eaten_at)} />
      )}
    </article>
  )
}

/** The meal's thumbnail; selecting it shows the full photo over the page. */
function MealPhotoKey({
  photo,
  time,
}: {
  readonly photo: MealPhoto
  readonly time: string
}): React.JSX.Element {
  const [open, setOpen] = useState(false)

  return (
    <>
      <button
        type="button"
        className="food-photo"
        aria-label={`Show the photo of the meal at ${time}`}
        onClick={() => setOpen(true)}
      >
        <img
          src={mealPhotoUrl(photo, "thumbnail")}
          width={photo.thumbnail.width}
          height={photo.thumbnail.height}
          alt=""
          loading="lazy"
          decoding="async"
        />
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent variant="fullscreen" showCloseButton={false}>
          <div className="photo-viewer food-photo-viewer">
            <DialogTitle className="sr-only">Meal at {time}</DialogTitle>
            <div className="photo-stage">
              <img
                src={mealPhotoUrl(photo, "photo")}
                width={photo.width}
                height={photo.height}
                alt={`The meal at ${time}`}
                decoding="async"
              />
            </div>
            <div className="photo-viewer-bar">
              <span className="photo-viewer-where">Meal at {time}</span>
              <DialogClose
                render={<Button variant="outline" size="key-icon" />}
                title="Close photo"
              >
                <XIcon size={22} aria-hidden="true" />
                <span className="sr-only">Close photo</span>
              </DialogClose>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}

function MealDetails({ meal }: { readonly meal: MealLog }): React.JSX.Element {
  return (
    <>
      <header>
        <h3>
          <time dateTime={meal.eaten_at}>{mealTime(meal.eaten_at)}</time>
        </h3>
        <Amount total={meal.totals.energy_kcal} />
      </header>
      {meal.meal_name === null ? null : (
        <p className="food-meal-from">
          <span className="sr-only">Logged from the Meal </span>
          {meal.meal_name}
        </p>
      )}
      {meal.note === null ? null : <p className="food-note">{meal.note}</p>}
      <ul className="food-portions" aria-label="Portions">
        {meal.portions.map((portion, index) => (
          <li key={`${portion.food_id}-${index}`}>
            <PortionTitle portion={portion} />
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
      {meal.supplement_doses.length === 0 ? null : (
        <ul className="food-doses" aria-label="Supplement Doses">
          {meal.supplement_doses.map((dose) => {
            const kcal = doseEnergy(dose)

            return (
              <li key={dose.supplement_id}>
                <span className="food-portion-name">{dose.name}</span>
                <span className="food-portion-grams num">
                  {doseCount(dose.doses, dose.dose_unit)}
                </span>
                <span className="food-portion-kcal num">
                  {kcal === null ? null : `${formatAmount(kcal)} kcal`}
                </span>
              </li>
            )
          })}
        </ul>
      )}
    </>
  )
}

/**
 * A Food's name, or a Product's label name with its brand beneath it, and the NOVA Class it was
 * logged with; an unclassified Portion has no class mark.
 */
function PortionTitle({
  portion,
}: {
  readonly portion: Portion
}): React.JSX.Element {
  const { name, brand } = portionName(portion)

  return (
    <span className="food-portion-name">
      {name}
      {portion.nova_class === null ? null : (
        <span
          className="food-portion-nova"
          data-nova={portion.nova_class}
          title={`NOVA ${portion.nova_class}: ${novaClassMeaning[portion.nova_class]}`}
        >
          <span className="sr-only">, </span>
          NOVA {portion.nova_class}
        </span>
      )}
      {brand === null ? null : (
        <span className="food-portion-brand">
          <span className="sr-only">, </span>
          {brand}
        </span>
      )}
    </span>
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

/** "2,120" kcal: targets are whole kcal. */
function kcal(value: number): string {
  return formatAmount(Math.round(value))
}

/**
 * The day's Calorie Target, eaten and remaining, with the goal and Estimated Expenditure behind
 * it, then protein, fat and carbs against their targets; or why there is no target yet.
 */
function TargetStrip({
  targets,
}: {
  readonly targets: DayTargets
}): React.JSX.Element {
  if (targets.state === "unavailable")
    return (
      <section
        className="food-targets"
        data-state="unavailable"
        aria-labelledby="food-targets-heading"
      >
        <h2 id="food-targets-heading">No Calorie Target yet</h2>
        <p>{targets.reason}.</p>
        <p className="food-targets-hint">
          Height, sex, birth date and Body Weight are set on the Workout
          page&apos;s Body view.
        </p>
      </section>
    )
  const over = targets.remaining_kcal < 0

  return (
    <section className="food-targets" aria-labelledby="food-targets-heading">
      <h2 id="food-targets-heading" className="sr-only">
        Calorie Target
      </h2>
      <dl className="food-target-strip">
        <div>
          <dt>Target</dt>
          <dd className="num">
            {kcal(targets.calorie_target_kcal)}
            <span className="food-unit"> kcal</span>
          </dd>
        </div>
        <div>
          <dt>Eaten</dt>
          <dd className="num">
            {kcal(targets.eaten_kcal)}
            <span className="food-unit"> kcal</span>
          </dd>
        </div>
        <div data-over={over ? "" : undefined}>
          <dt>{over ? "Over" : "Remaining"}</dt>
          <dd className="num">
            {kcal(Math.abs(targets.remaining_kcal))}
            <span className="food-unit"> kcal</span>
          </dd>
        </div>
      </dl>
      <p className="food-target-basis">
        {goalName(targets.goal)} · Estimated Expenditure{" "}
        {kcal(targets.estimated_expenditure_kcal)} kcal · Trend Weight{" "}
        {formatAmount(targets.trend_weight_kg)} kg
        {targets.bmr_floor_applied
          ? ` · held at BMR ${kcal(targets.bmr_kcal)} kcal`
          : ""}
      </p>
      <ul className="food-macro-targets" aria-label="Macro Targets">
        <MacroProgress
          macro="protein"
          name="Protein"
          eaten={targets.macros.protein.eaten_g}
          target={targets.macros.protein.target_g}
          status={targets.macros.protein.status === "met" ? "met" : "to eat"}
        />
        <MacroProgress
          macro="fat"
          name="Fat"
          eaten={targets.macros.fat.eaten_g}
          target={targets.macros.fat.floor_g}
          status={
            targets.macros.fat.status === "met" ? "floor met" : "below floor"
          }
        />
        <MacroProgress
          macro="available_carbohydrate"
          name="Carbs"
          eaten={targets.macros.carbs.eaten_g}
          target={targets.macros.carbs.target_g}
          status={null}
        />
      </ul>
    </section>
  )
}

/** One macro eaten against its target as a figure and a bar; carbs carry no status. */
function MacroProgress({
  macro,
  name,
  eaten,
  target,
  status,
}: {
  readonly macro: "protein" | "fat" | "available_carbohydrate"
  readonly name: string
  readonly eaten: number
  readonly target: number
  readonly status: "met" | "to eat" | "floor met" | "below floor" | null
}): React.JSX.Element {
  const share = target > 0 ? Math.min(eaten / target, 1) : 1

  return (
    <li data-macro={macro} data-status={status ?? undefined}>
      <span className="food-macro-target-name">{name}</span>
      <span className="food-macro-target-amount num">
        {formatAmount(eaten)} / {formatAmount(target)}
        <span className="food-unit"> g</span>
      </span>
      <span className="food-macro-target-status">{status ?? ""}</span>
      <span
        className="food-macro-target-bar"
        aria-hidden="true"
        // SAFETY: CSSProperties omits custom properties; --share is the eaten share of the target.
        style={{ "--share": share } as React.CSSProperties}
      />
    </li>
  )
}

/** The Nutrition Goal from today: the Day view's only write. */
function GoalControl({
  goal,
  save,
  reload,
}: {
  readonly goal: NutritionGoal | null
  readonly save: FoodDayProps["saveGoal"]
  readonly reload: FoodDayProps["reload"]
}): React.JSX.Element {
  const id = useId()
  const current = goal === null ? undefined : goalChoice(goal.goal)
  const [chosen, setChosen] = useState<string>(current?.key ?? "")
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)

  const [feedback, setFeedback] = useState<{
    readonly failed: boolean
    readonly text: string
  }>()

  if (goal === null)
    return (
      <p className="food-goal" role="status">
        The Nutrition Goal could not be read. Reopen the page to change it.
      </p>
    )

  async function submit(): Promise<void> {
    const choice = goalChoices.find((option) => option.key === chosen)

    if (choice === undefined || locked.current) return
    locked.current = true
    setBusy(true)
    setFeedback(undefined)

    try {
      const result = await save({
        direction: choice.direction,
        rate_kg_per_week: choice.rate_kg_per_week,
      })

      if (result.state === "failed") {
        setFeedback({
          failed: true,
          text: nutritionFailureMessage[result.reason],
        })

        return
      }

      await reload()
      setFeedback({
        failed: false,
        text: `Goal saved: ${goalName(choice)} from today.`,
      })
    } catch {
      setFeedback({ failed: true, text: nutritionFailureMessage.unreachable })
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  return (
    <form
      className="food-goal"
      onSubmit={(event) => {
        event.preventDefault()
        void submit()
      }}
    >
      <label htmlFor={`${id}-goal`}>Goal from today</label>
      <select
        id={`${id}-goal`}
        value={chosen}
        disabled={busy}
        onChange={(event) => {
          setChosen(event.target.value)
          setFeedback(undefined)
        }}
      >
        {current === undefined ? (
          <option value="">{goalName(goal.goal)}</option>
        ) : null}
        {goalChoices.map((choice) => (
          <option key={choice.key} value={choice.key}>
            {goalName(choice)}
          </option>
        ))}
      </select>
      <Button
        type="submit"
        variant="outline"
        disabled={busy || chosen === "" || chosen === current?.key}
      >
        Save goal
      </Button>
      <p
        className="food-goal-feedback"
        role={feedback?.failed === true ? "alert" : "status"}
        data-failed={feedback?.failed}
      >
        {feedback?.text ?? ""}
      </p>
    </form>
  )
}
