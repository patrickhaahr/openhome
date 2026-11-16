import { useId, useRef, useState } from "react"
import { PersonSimpleRunIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleRun"
import { PersonSimpleWalkIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleWalk"
import { FootprintsIcon } from "@phosphor-icons/react/dist/csr/Footprints"
import { useIdleBlock } from "@/components/idle-block"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import type { MapRoutes } from "@/lib/map"
import {
  activeRouteMap,
  type ActiveRoute,
  type ActiveRouteRead,
  type RouteSelectionRead,
  type LoopRead,
  type LoopRequest,
} from "@/lib/route-loop"
import {
  alternativesMap,
  LETTERS,
  type Shown,
  type Status,
  type SelectionStatus,
  ActiveSelection,
  StatusMessage,
  LoopAlternatives,
} from "@/components/loop-results"
import {
  INITIAL_FORM,
  isTargetChoice,
  readForm,
  type Field,
  type Form,
} from "@/components/loop-form"

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
  // Planning or saving the Active Route is declared busy.
  useIdleBlock(
    status.state === "planning" ||
      selection.state === "saving" ||
      selection.state === "checking"
  )

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

  // The plate only exists on screen while it has something to say; otherwise its
  // words stay with assistive technology.
  const plateShown =
    shown !== undefined ||
    active.state !== "none" ||
    status.state === "failed" ||
    status.state === "incomplete" ||
    selection.state !== "idle"

  return (
    <div className="loop-dock" aria-busy={busy}>
      <h2 className="sr-only">Walk or run from here</h2>
      <div
        key={`${plateShown}:${shown !== undefined}`}
        className={plateShown ? "loop-plate" : "sr-only"}
      >
        <ActiveSelection
          active={active}
          status={selection}
          busy={busy}
          onCheck={() => void checkSelection()}
          onShow={() => {
            if (active.state === "selected") showActive(active.route)
          }}
        />
        <div role="status" className="loop-planner-status">
          <StatusMessage status={status} shown={shown} problemId={problemId} />
        </div>
        {shown === undefined ? null : (
          <>
            <LoopAlternatives shown={shown} onFocus={focus} disabled={saving} />
            <Button
              type="button"
              size="key"
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
              Comparing loops does not change the route selected for your phone.
            </p>
          </>
        )}
      </div>
      <form
        className="loop-bar"
        onSubmit={(event) => {
          event.preventDefault()
          void planLoops()
        }}
      >
        <fieldset disabled={busy}>
          <legend className="sr-only">Activity</legend>
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
              <span className="sr-only">Walk</span>
              <PersonSimpleWalkIcon size={24} aria-hidden="true" />
            </ToggleGroupItem>
            <ToggleGroupItem value="running" size="lg">
              <span className="sr-only">Run</span>
              <PersonSimpleRunIcon size={24} aria-hidden="true" />
            </ToggleGroupItem>
          </ToggleGroup>
        </fieldset>
        <fieldset disabled={busy}>
          <legend className="sr-only">Target</legend>
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
          {form.target === "distance" ? (
            <div className="loop-field">
              <label htmlFor={`${id}-kilometers`} className="sr-only">
                Distance in km
              </label>
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
              <span className="loop-field-unit" aria-hidden="true">
                km
              </span>
            </div>
          ) : null}
          {form.target === "steps" ? (
            <div className="loop-field">
              <label htmlFor={`${id}-steps`} className="sr-only">
                Steps
              </label>
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
              <span className="loop-field-unit" aria-hidden="true">
                steps
              </span>
            </div>
          ) : null}
        </fieldset>
        <div className="loop-field">
          <label htmlFor={`${id}-step-length`} className="sr-only">
            {form.activity === "running" ? "Running" : "Walking"} step length in
            cm
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
          <span className="loop-field-unit" aria-hidden="true">
            cm
          </span>
          <p id={`${id}-step-length-hint`} className="sr-only">
            Converts steps to distance. Step counts are estimates.
          </p>
        </div>
        <Button type="submit" size="key" disabled={busy}>
          {status.state === "planning" ? "Planning…" : "Plan loops"}
        </Button>
      </form>
    </div>
  )
}
