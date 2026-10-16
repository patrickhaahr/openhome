import { useId, useRef, useState } from "react"
import { PersonSimpleRunIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleRun"
import { PersonSimpleWalkIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleWalk"
import { DeviceMobileIcon } from "@phosphor-icons/react/dist/csr/DeviceMobile"
import { FootprintsIcon } from "@phosphor-icons/react/dist/csr/Footprints"
import { MapPinAreaIcon } from "@phosphor-icons/react/dist/csr/MapPinArea"
import { WarningCircleIcon } from "@phosphor-icons/react/dist/csr/WarningCircle"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import type { MapLine, MapRoutes } from "@/lib/map"
import {
  activeRouteMap,
  describeTarget,
  type ActiveRoute,
  type ActiveRouteRead,
  type RouteSelectionRead,
  describeDeviation,
  describeEstimatedSteps,
  describeFailure,
  describePlan,
  describeCount,
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

/** The typed input a problem belongs to. */
type Field = "kilometers" | "steps" | "step-length"

/** What is missing before a request can be sent, and where. */
type Problem = { readonly field: Field; readonly message: string }

/** The request the form describes, or what is missing before one can be sent. */
type FormRead =
  | { readonly state: "ready"; readonly request: LoopRequest }
  | ({ readonly state: "incomplete" } & Problem)

/** A failed request: Axum's outcome, a rejected target or a lost connection. */
type Failure = Exclude<LoopRead, { state: "generated" }>

/** The last request: nothing asked yet, in flight, answered, failed or not sendable. */
type Status =
  | { readonly state: "idle" | "planning" | "planned" }
  | { readonly state: "failed"; readonly failure: Failure }
  | ({ readonly state: "incomplete" } & Problem)

/** The loops on screen; they stay usable when a later request fails. */
type Shown = {
  readonly plan: LoopPlan
  readonly lines: ReadonlyArray<MapLine>
  readonly focused: number | undefined
}

const LETTERS = ["A", "B", "C"]

/**
 * Plan walking or running loops from the shared map origin by distance or
 * steps, then compare the measured alternatives on the map. Drawn routes always
 * come from the provider's geometry.
 */
export function LoopPlanner({
  plan,
  initialActive,
  select,
  reloadActive,
  onRoutesChange,
}: {
  plan: (request: LoopRequest) => Promise<LoopRead>
  initialActive: ActiveRouteRead
  select: (routeId: string) => Promise<RouteSelectionRead>
  reloadActive: () => Promise<ActiveRouteRead>
  onRoutesChange: (routes: MapRoutes) => void
}): React.JSX.Element {
  const id = useId()
  const [active, setActive] = useState(initialActive)
  const [selection, setSelection] = useState<SelectionStatus>({ state: "idle" })
  const writing = useRef(false)
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
    onRoutesChange(alternativesMap(next, 0))
  }

  function focus(index: number): void {
    if (shown === undefined) return

    setShown({ ...shown, focused: index })
    onRoutesChange(alternativesMap(shown, index))
  }

  async function choose(): Promise<void> {
    const route =
      shown?.focused === undefined ? undefined : shown.plan.loops[shown.focused]

    if (route === undefined || writing.current) return

    writing.current = true
    setSelection({ state: "saving" })
    const result = await select(route.id)
    writing.current = false

    if (result.state === "selected") {
      setActive(result)
      setSelection({ state: "saved" })
    } else setSelection(result)
  }

  async function checkSelection(): Promise<void> {
    if (writing.current) return

    writing.current = true
    setSelection({ state: "checking" })
    const result = await reloadActive()
    writing.current = false

    if (result.state === "unavailable") {
      setSelection({ state: "failed", reason: "connection-unavailable" })
    } else {
      setActive(result)
      setSelection({ state: "idle" })

      if (result.state === "selected") showActive(result.route)
    }
  }

  const saving = selection.state === "saving" || selection.state === "checking"
  const busy = status.state === "planning" || saving

  function showActive(route: ActiveRoute): void {
    if (shown !== undefined) {
      const index = shown.plan.loops.findIndex(
        (candidate) => candidate.id === route.id
      )

      setShown({ ...shown, focused: index < 0 ? undefined : index })
    }

    onRoutesChange(activeRouteMap(route))
  }

  const candidate =
    shown?.focused === undefined ? undefined : shown.plan.loops[shown.focused]

  const alreadySelected =
    active.state === "selected" && active.route.id === candidate?.id

  const problemId = `${id}-problem`

  /** Marks the input a problem belongs to and describes it with the message. */
  function problemFor(
    field: Field,
    hint?: string
  ): { "aria-invalid": boolean; "aria-describedby"?: string } {
    const invalid = status.state === "incomplete" && status.field === field

    const describedBy = [hint, invalid ? problemId : undefined]
      .filter((part) => part !== undefined)
      .join(" ")

    return describedBy === ""
      ? { "aria-invalid": invalid }
      : { "aria-invalid": invalid, "aria-describedby": describedBy }
  }

  return (
    <Card aria-busy={busy}>
      <CardContent>
        <h2 className="sr-only">Walk or run from here</h2>
        <div className="loop-planner-body">
          <ActiveSelection
            active={active}
            status={selection}
            busy={busy}
            onCheck={() => void checkSelection()}
            onShow={() => {
              if (active.state === "selected") showActive(active.route)
            }}
          />
          <form
            className="loop-planner-form"
            onSubmit={(event) => {
              event.preventDefault()
              void planLoops()
            }}
          >
            <fieldset disabled={busy}>
              <legend className="sr-only">Activity</legend>
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
                    <PersonSimpleWalkIcon size={20} aria-hidden="true" />
                  </ToggleGroupItem>
                  <ToggleGroupItem value="running" size="lg">
                    Run
                    <PersonSimpleRunIcon size={20} aria-hidden="true" />
                  </ToggleGroupItem>
                </ToggleGroup>
              </div>
            </fieldset>
            <fieldset disabled={busy}>
              <legend className="sr-only">Target</legend>
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
                    {...problemFor("kilometers")}
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
                    {...problemFor("steps")}
                    value={form.steps}
                    onChange={(event) =>
                      setForm({ ...form, steps: event.target.value })
                    }
                  />
                </div>
              ) : null}
            </fieldset>
            <div className="loop-planner-field loop-planner-step">
              <label htmlFor={`${id}-step-length`} className="sr-only">
                {form.activity === "running" ? "Running" : "Walking"} step
                length in cm
              </label>
              <FootprintsIcon size={22} aria-hidden="true" />
              <Input
                id={`${id}-step-length`}
                inputMode="numeric"
                autoComplete="off"
                disabled={busy}
                {...problemFor("step-length", `${id}-step-length-hint`)}
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
              <span className="loop-planner-unit" aria-hidden="true">
                cm
              </span>
              <p id={`${id}-step-length-hint`} className="sr-only">
                Converts steps to distance. Step counts are estimates.
              </p>
            </div>
            <Button type="submit" size="lg" disabled={busy}>
              {status.state === "planning" ? "Planning…" : "Plan loops"}
            </Button>
          </form>
          <div role="status" className="loop-planner-status">
            <StatusMessage
              status={status}
              shown={shown}
              problemId={problemId}
            />
          </div>
          {shown === undefined ? null : (
            <>
              <LoopAlternatives
                shown={shown}
                onFocus={focus}
                disabled={saving}
              />
              <Button
                type="button"
                size="lg"
                disabled={busy || alreadySelected || candidate === undefined}
                onClick={() => void choose()}
              >
                {selection.state === "saving"
                  ? "Selecting…"
                  : shown.focused === undefined
                    ? "Choose a loop to select"
                    : alreadySelected
                      ? "Active route selected"
                      : `Select Loop ${LETTERS[shown.focused] ?? shown.focused + 1}`}
              </Button>
              <p className="sr-only">
                Comparing loops does not change the route selected for your
                phone.
              </p>
            </>
          )}
          <p className="loop-planner-attribution">
            Routes by openrouteservice · Map data © OpenStreetMap contributors
          </p>
        </div>
      </CardContent>
    </Card>
  )
}

function alternativesMap(shown: Shown, index: number): MapRoutes {
  const route = shown.plan.loops[index]

  return {
    lines: shown.lines,
    focused: index,
    label:
      route === undefined
        ? "Showing loop alternatives"
        : `Showing Loop ${LETTERS[index] ?? index + 1} · ${formatKilometers(route.distance_meters)}`,
  }
}

/** Saving and checking are mutually exclusive; errors never claim a replacement was saved. */
type SelectionStatus =
  | { readonly state: "idle" | "saving" | "saved" | "checking" }
  | Extract<RouteSelectionRead, { state: "failed" }>

function ActiveSelection({
  active,
  status,
  busy,
  onCheck,
  onShow,
}: {
  active: ActiveRouteRead
  status: SelectionStatus
  busy: boolean
  onCheck: () => void
  onShow: () => void
}): React.JSX.Element {
  return (
    <section
      className="loop-active"
      aria-label="Active route"
      data-state={active.state}
    >
      {active.state === "selected" ? (
        <>
          <DeviceMobileIcon size={22} aria-hidden="true" />
          <h3>
            {formatKilometers(active.route.distance_meters)}{" "}
            <span className="sr-only">{active.route.activity} loop</span>
          </h3>
          <div className="sr-only">
            <p>
              Target: {describeTarget(active.route.target)} · About{" "}
              {describeCount(active.route.estimated_steps, "step")}, estimated
              from a {active.route.step_length_cm} cm step.
            </p>
            <p>Selected for your phone. A walk or run starts on the phone.</p>
          </div>
          <Button
            type="button"
            variant="outline"
            size="key-icon"
            disabled={busy}
            title="Show active route on map"
            onClick={onShow}
          >
            <MapPinAreaIcon size={20} aria-hidden="true" />
            <span className="sr-only">Show active route on map</span>
          </Button>
        </>
      ) : (
        <p
          className={
            active.state === "none" ? "sr-only" : "loop-planner-problem"
          }
        >
          {active.state === "none" ? null : (
            <WarningCircleIcon size={20} weight="fill" aria-hidden="true" />
          )}
          <span className={active.state === "none" ? undefined : "sr-only"}>
            {active.state === "none"
              ? "No route selected yet. Compare loops, then select one for your phone."
              : "Could not read the selected route. An earlier selection may still be saved on OpenHome."}
          </span>
        </p>
      )}
      <div className="loop-active-status" role="status" aria-live="polite">
        {status.state === "saving" ? <p>Saving the selected loop…</p> : null}
        {status.state === "checking" ? (
          <p>Checking the selected route…</p>
        ) : null}
        {status.state === "saved" ? (
          <p>Loop selected. Your phone can retrieve it.</p>
        ) : null}
        {status.state === "failed" ? (
          <p className="loop-planner-problem">
            {status.reason === "candidate-unavailable"
              ? "This loop is no longer available. Plan loops again to select a route."
              : "Selection could not be confirmed. Check the saved route or try selecting again."}{" "}
            {active.state === "selected"
              ? "The last confirmed active route is still shown."
              : ""}
          </p>
        ) : null}
      </div>
      {active.state === "unavailable" || status.state === "failed" ? (
        <Button
          type="button"
          variant="outline"
          disabled={busy}
          onClick={onCheck}
        >
          Check selection
        </Button>
      ) : null}
    </section>
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
      field: "step-length",
      message: "Enter the step length in whole centimetres.",
    }

  return {
    state: "ready",
    request: { activity: form.activity, target, step_length_cm: stepLength },
  }
}

function readTarget(form: Form): LoopRequest["target"] | Problem {
  switch (form.target) {
    case "5k":
      return { kind: "distance", meters: 5000 }
    case "10k":
      return { kind: "distance", meters: 10_000 }
    case "distance": {
      const text = form.kilometers.trim().replace(",", ".")

      const meters = /^\d+(\.\d+)?$/.test(text)
        ? Math.round(Number(text) * 1000)
        : 0

      return Number.isSafeInteger(meters) && meters > 0
        ? { kind: "distance", meters }
        : { field: "kilometers", message: "Enter a distance in kilometres." }
    }

    case "steps": {
      const steps = wholeNumber(form.steps)

      return steps === undefined
        ? { field: "steps", message: "Enter a whole number of steps." }
        : { kind: "steps", steps }
    }
  }
}

/** A positive whole number typed with digits only, or `undefined`. */
function wholeNumber(text: string): number | undefined {
  const trimmed = text.trim()
  const value = /^\d+$/.test(trimmed) ? Number(trimmed) : 0

  return Number.isSafeInteger(value) && value > 0 ? value : undefined
}

function StatusMessage({
  status,
  shown,
  problemId,
}: {
  status: Status
  shown: Shown | undefined
  problemId: string
}): React.JSX.Element | null {
  switch (status.state) {
    case "idle":
      return (
        <p className="sr-only">
          Up to three loops on footways and streets that return to the origin.
        </p>
      )
    case "planning":
      return <p className="sr-only">Looking for loops…</p>
    case "planned":
      return shown === undefined ? null : (
        <p className="sr-only">
          Found {describeCount(shown.plan.loops.length, "loop")}. Shown on the
          map; each starts and ends near the origin.
          <ShortfallAnnouncement plan={shown.plan} />
        </p>
      )
    case "incomplete":
      return (
        <p id={problemId} className="loop-planner-problem">
          {status.message}
        </p>
      )
    case "failed": {
      const { title, detail } = describeFailure(status.failure)

      return (
        <div className="loop-planner-failure">
          <h3>
            <WarningCircleIcon size={20} weight="fill" aria-hidden="true" />
            {title}
          </h3>
          <p className="sr-only">{detail}</p>
          {shown === undefined ? null : (
            <p className="sr-only">
              Still showing the earlier {describePlan(shown.plan)}.
            </p>
          )}
        </div>
      )
    }
  }
}

/** Announces a shortfall with the result; each loop also marks its own fit. */
function ShortfallAnnouncement({
  plan,
}: {
  plan: LoopPlan
}): React.JSX.Element | null {
  const shortfall = describeShortfall(plan)

  return shortfall === undefined ? null : (
    <span className="sr-only"> {shortfall}</span>
  )
}

function LoopAlternatives({
  shown,
  onFocus,
  disabled,
}: {
  disabled: boolean
  shown: Shown
  onFocus: (index: number) => void
}): React.JSX.Element {
  const id = useId()
  const { plan } = shown

  return (
    <section className="loop-alternatives" aria-labelledby={id}>
      <h3 id={id} className="sr-only">
        {capitalize(describePlan(plan))}
      </h3>
      <ul>
        {plan.loops.map((route, index) => (
          <li key={route.id}>
            <Button
              type="button"
              variant="loop"
              className="h-auto"
              aria-pressed={index === shown.focused}
              data-fit={route.within_tolerance ? "within" : "outside"}
              disabled={disabled}
              onClick={() => onFocus(index)}
            >
              <span className="loop-alternative-name">
                Loop {LETTERS[index] ?? index + 1}
              </span>
              <span className="loop-alternative-distance">
                {formatKilometers(route.distance_meters)}
              </span>
              <span className="loop-alternative-fit" aria-hidden="true" />
              <span className="sr-only">
                {describeDeviation({ route, plan })}
              </span>
              <span className="sr-only">
                {describeEstimatedSteps({ route, plan })}
              </span>
            </Button>
          </li>
        ))}
      </ul>
    </section>
  )
}

function capitalize(text: string): string {
  return `${text.charAt(0).toUpperCase()}${text.slice(1)}`
}
