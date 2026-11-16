import { useId, useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import {
  goalChoice,
  goalChoices,
  goalName,
  formatAmount,
  nutritionFailureMessage,
  type DayTargets,
  type GoalMutation,
  type NutritionGoal,
  type SetGoalInput,
} from "@/lib/nutrition"

/** "2,120" kcal: targets are whole kcal. */
function kcal(value: number): string {
  return formatAmount(Math.round(value))
}

/**
 * The day's Calorie Target, eaten and remaining, with the goal and Estimated Expenditure behind
 * it, then protein, fat and carbs against their targets; or why there is no target yet.
 */
export function TargetStrip({
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

/** Identifies the goal Axum serves; null when it could not be read. */
function goalSignature(goal: NutritionGoal | null): string | null {
  if (goal === null) return null
  const { direction, rate_kg_per_week, effective_from } = goal.goal

  return `${direction} ${rate_kg_per_week} ${effective_from ?? ""}`
}

/** The Nutrition Goal from today: the Day view's only write. */
export function GoalControl({
  goal,
  save,
  reload,
}: {
  readonly goal: NutritionGoal | null
  readonly save: (goal: SetGoalInput) => Promise<GoalMutation>
  readonly reload: () => Promise<void>
}): React.JSX.Element {
  const id = useId()
  const current = goal === null ? undefined : goalChoice(goal.goal)
  const [chosen, setChosen] = useState<string>(current?.key ?? "")
  // The choice starts again from the goal Axum serves whenever that goal changes, such as when
  // it could first not be read; adjusting during render keeps the save feedback.
  const signature = goalSignature(goal)
  const [shown, setShown] = useState(signature)

  if (shown !== signature) {
    setShown(signature)
    setChosen(current?.key ?? "")
  }

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
