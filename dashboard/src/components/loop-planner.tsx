import { useId, useState } from "react"
import { PersonSimpleRunIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleRun"
import { PersonSimpleWalkIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleWalk"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import type { MapLine, MapRoutes } from "@/lib/map"
import {
  describeDeviation,
  describeEstimatedSteps,
  describeFailure,
  describePlan,
  describeShortfall,
  formatKilometers,
  type Activity,
  type LoopPlan,
  type LoopRead,
  type LoopRequest,
} from "@/lib/route-loop"

/** The targets offered: two presets, a custom distance or a step count. */
type TargetChoice = "5k" | "10k" | "distance" | "steps"

/** The planner's inputs as typed; Axum decides which targets it supports. */
type Form = {
  readonly activity: Activity
  readonly target: TargetChoice
  readonly kilometers: string
  readonly steps: string
  /** A running stride is longer than a walking step, so each keeps its own. */
  readonly stepLength: Readonly<Record<Activity, string>>
}

const INITIAL_FORM: Form = {
  activity: "walking",
  target: "5k",
  kilometers: "7.5",
  steps: "10000",
  stepLength: { walking: "75", running: "100" },
}

/** The request the form describes, or what is missing before one can be sent. */
type FormRead =
  | { readonly state: "ready"; readonly request: LoopRequest }
  | { readonly state: "incomplete"; readonly message: string }

/** A failed request: Axum's outcome, a rejected target or a lost connection. */
type Failure = Exclude<LoopRead, { state: "generated" }>

/** The last request: nothing asked yet, in flight, answered, failed or not sendable. */
type Status =
  | { readonly state: "idle" | "planning" | "planned" }
  | { readonly state: "failed"; readonly failure: Failure }
  | { readonly state: "incomplete"; readonly message: string }

/** The loops on screen; they stay usable when a later request fails. */
type Shown = {
  readonly plan: LoopPlan
  readonly lines: ReadonlyArray<MapLine>
  readonly focused: number
}

const LETTERS = ["A", "B", "C", "D", "E", "F"]

/**
 * Plan walking or running loops from the shared map origin by distance or
 * steps, then compare the measured alternatives on the map. Drawn routes always
 * come from the provider's geometry.
 */
export function LoopPlanner({
  plan,
  onRoutesChange,
}: {
  plan: (request: LoopRequest) => Promise<LoopRead>
  onRoutesChange: (routes: MapRoutes) => void
}): React.JSX.Element {
  const id = useId()
  const [form, setForm] = useState<Form>(INITIAL_FORM)
  const [status, setStatus] = useState<Status>({ state: "idle" })
  const [shown, setShown] = useState<Shown | undefined>(undefined)

  async function planLoops(): Promise<void> {
    const read = readForm(form)

    if (read.state === "incomplete") {
      setStatus(read)

      return
    }

    setStatus({ state: "planning" })
    const result = await plan(read.request)

    if (result.state !== "generated") {
      // Earlier loops stay on screen and on the map, labelled with their own target.
      setStatus({ state: "failed", failure: result })

      return
    }

    const next: Shown = {
      plan: result.plan,
      lines: result.plan.loops.map((route) => route.geometry),
      focused: 0,
    }

    setShown(next)
    setStatus({ state: "planned" })
    onRoutesChange({ lines: next.lines, focused: next.focused })
  }

  function focus(index: number): void {
    if (shown === undefined) return

    setShown({ ...shown, focused: index })
    onRoutesChange({ lines: shown.lines, focused: index })
  }

  const busy = status.state === "planning"

  return (
    <Card aria-busy={busy}>
      <CardHeader>
        <p className="loop-planner-eyebrow">
          {form.activity === "running" ? (
            <PersonSimpleRunIcon size={20} aria-hidden="true" />
          ) : (
            <PersonSimpleWalkIcon size={20} aria-hidden="true" />
          )}
          Loop planner
        </p>
        <h2>Walk or run from the origin</h2>
      </CardHeader>
      <CardContent>
        <div className="loop-planner-body">
          <form
            className="loop-planner-form"
            onSubmit={(event) => {
              event.preventDefault()
              void planLoops()
            }}
          >
            <fieldset disabled={busy}>
              <legend>Activity</legend>
              <div className="loop-planner-choices">
                <ToggleGroup
                  variant="outline"
                  spacing={0}
                  value={[form.activity]}
                  onValueChange={([next]) => {
                    if (next === "walking" || next === "running")
                      setForm({ ...form, activity: next })
                  }}
                >
                  <ToggleGroupItem value="walking" size="lg">
                    Walk
                  </ToggleGroupItem>
                  <ToggleGroupItem value="running" size="lg">
                    Run
                  </ToggleGroupItem>
                </ToggleGroup>
              </div>
            </fieldset>
            <fieldset disabled={busy}>
              <legend>Target</legend>
              <div className="loop-planner-choices">
                <ToggleGroup
                  variant="outline"
                  spacing={0}
                  value={[form.target]}
                  onValueChange={([next]) => {
                    if (isTargetChoice(next)) setForm({ ...form, target: next })
                  }}
                >
                  <ToggleGroupItem value="5k" size="lg">
                    5 km
                  </ToggleGroupItem>
                  <ToggleGroupItem value="10k" size="lg">
                    10 km
                  </ToggleGroupItem>
                  <ToggleGroupItem value="distance" size="lg">
                    Custom
                  </ToggleGroupItem>
                  <ToggleGroupItem value="steps" size="lg">
                    Steps
                  </ToggleGroupItem>
                </ToggleGroup>
              </div>
              {form.target === "distance" ? (
                <div className="loop-planner-field">
                  <label htmlFor={`${id}-kilometers`}>Distance in km</label>
                  <Input
                    id={`${id}-kilometers`}
                    inputMode="decimal"
                    autoComplete="off"
                    value={form.kilometers}
                    onChange={(event) =>
                      setForm({ ...form, kilometers: event.target.value })
                    }
                  />
                </div>
              ) : null}
              {form.target === "steps" ? (
                <div className="loop-planner-field">
                  <label htmlFor={`${id}-steps`}>Steps</label>
                  <Input
                    id={`${id}-steps`}
                    inputMode="numeric"
                    autoComplete="off"
                    value={form.steps}
                    onChange={(event) =>
                      setForm({ ...form, steps: event.target.value })
                    }
                  />
                </div>
              ) : null}
            </fieldset>
            <div className="loop-planner-field">
              <label htmlFor={`${id}-step-length`}>
                {form.activity === "running" ? "Running" : "Walking"} step
                length in cm
              </label>
              <Input
                id={`${id}-step-length`}
                inputMode="numeric"
                autoComplete="off"
                disabled={busy}
                aria-describedby={`${id}-step-length-hint`}
                value={form.stepLength[form.activity]}
                onChange={(event) =>
                  setForm({
                    ...form,
                    stepLength: {
                      ...form.stepLength,
                      [form.activity]: event.target.value,
                    },
                  })
                }
              />
              <p id={`${id}-step-length-hint`} className="loop-planner-hint">
                Converts steps to distance. Step counts are estimates.
              </p>
            </div>
            <Button type="submit" size="lg" disabled={busy}>
              {busy ? "Planning…" : "Plan loops"}
            </Button>
          </form>
          <div role="status" className="loop-planner-status">
            <StatusMessage status={status} shown={shown} />
          </div>
          {shown === undefined ? null : (
            <LoopAlternatives shown={shown} onFocus={focus} />
          )}
          <p className="loop-planner-attribution">
            Routes by openrouteservice · Map data © OpenStreetMap contributors
          </p>
        </div>
      </CardContent>
    </Card>
  )
}

function isTargetChoice(value: unknown): value is TargetChoice {
  return (
    value === "5k" ||
    value === "10k" ||
    value === "distance" ||
    value === "steps"
  )
}

/** Turn the typed inputs into a request; Axum still checks the ranges. */
function readForm(form: Form): FormRead {
  const target = readTarget(form)

  if ("message" in target) return { state: "incomplete", ...target }

  const stepLength = wholeNumber(form.stepLength[form.activity])

  if (stepLength === undefined)
    return {
      state: "incomplete",
      message: "Enter the step length in whole centimetres.",
    }

  return {
    state: "ready",
    request: { activity: form.activity, target, step_length_cm: stepLength },
  }
}

function readTarget(
  form: Form
): LoopRequest["target"] | { readonly message: string } {
  switch (form.target) {
    case "5k":
      return { kind: "distance", meters: 5000 }
    case "10k":
      return { kind: "distance", meters: 10_000 }
    case "distance": {
      const text = form.kilometers.trim().replace(",", ".")
      const kilometers = /^\d+(\.\d+)?$/.test(text) ? Number(text) : 0

      return kilometers > 0
        ? { kind: "distance", meters: Math.round(kilometers * 1000) }
        : { message: "Enter a distance in kilometres." }
    }

    case "steps": {
      const steps = wholeNumber(form.steps)

      return steps === undefined
        ? { message: "Enter a whole number of steps." }
        : { kind: "steps", steps }
    }
  }
}

/** A positive whole number typed with digits only, or `undefined`. */
function wholeNumber(text: string): number | undefined {
  const trimmed = text.trim()
  const value = /^\d+$/.test(trimmed) ? Number(trimmed) : 0

  return value > 0 ? value : undefined
}

function StatusMessage({
  status,
  shown,
}: {
  status: Status
  shown: Shown | undefined
}): React.JSX.Element | null {
  switch (status.state) {
    case "idle":
      return (
        <p className="loop-planner-hint">
          Up to three loops on footways and streets that return to the origin.
        </p>
      )
    case "planning":
      return <p className="loop-planner-hint">Looking for loops…</p>
    case "planned":
      return shown === undefined ? null : (
        <p className="loop-planner-hint">
          Found{" "}
          {shown.plan.loops.length === 1
            ? "1 loop"
            : `${shown.plan.loops.length} loops`}
          . Shown on the map; each starts and ends near the origin.
        </p>
      )
    case "incomplete":
      return <p className="loop-planner-problem">{status.message}</p>
    case "failed": {
      const { title, detail } = describeFailure(status.failure)

      return (
        <div className="loop-planner-failure">
          <h3>{title}</h3>
          <p className="loop-planner-hint">{detail}</p>
          {shown === undefined ? null : (
            <p className="loop-planner-hint">
              Still showing the earlier {describePlan(shown.plan)}.
            </p>
          )}
        </div>
      )
    }
  }
}

function LoopAlternatives({
  shown,
  onFocus,
}: {
  shown: Shown
  onFocus: (index: number) => void
}): React.JSX.Element {
  const id = useId()
  const { plan } = shown
  const shortfall = describeShortfall(plan)

  return (
    <section className="loop-alternatives" aria-labelledby={id}>
      <h3 id={id}>{capitalize(describePlan(plan))}</h3>
      {shortfall === undefined ? null : (
        <p className="loop-planner-shortfall">{shortfall}</p>
      )}
      <ul>
        {plan.loops.map((route, index) => (
          <li key={index}>
            <button
              type="button"
              className="loop-alternative"
              aria-pressed={index === shown.focused}
              data-fit={route.within_tolerance ? "within" : "outside"}
              onClick={() => onFocus(index)}
            >
              <span className="loop-alternative-name">
                Loop {LETTERS[index] ?? index + 1}
              </span>
              <span className="loop-alternative-distance">
                {formatKilometers(route.distance_meters)}
              </span>
              <span className="loop-alternative-fit">
                {describeDeviation({ route, plan })}
              </span>
              <span className="loop-alternative-steps">
                {describeEstimatedSteps({ route, plan })}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  )
}

function capitalize(text: string): string {
  return `${text.charAt(0).toUpperCase()}${text.slice(1)}`
}
