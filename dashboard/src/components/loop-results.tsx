import { useId } from "react"
import { ApproximateEqualsIcon } from "@phosphor-icons/react/dist/csr/ApproximateEquals"
import { CheckIcon } from "@phosphor-icons/react/dist/csr/Check"
import { DeviceMobileIcon } from "@phosphor-icons/react/dist/csr/DeviceMobile"
import { MapPinAreaIcon } from "@phosphor-icons/react/dist/csr/MapPinArea"
import { WarningCircleIcon } from "@phosphor-icons/react/dist/csr/WarningCircle"
import { Button } from "@/components/ui/button"
import type { MapLine, MapRoutes } from "@/lib/map"
import type { Problem } from "@/components/loop-form"
import {
  describeTarget,
  type ActiveRouteRead,
  type RouteSelectionRead,
  describeDeviation,
  describeEstimatedSteps,
  describeFailure,
  describePlan,
  describeCount,
  describeShortfall,
  formatKilometers,
  type LoopPlan,
  type LoopRead,
} from "@/lib/route-loop"

/** A failed request: Axum's outcome, a rejected target or a lost connection. */
type Failure = Exclude<LoopRead, { state: "generated" }>

/** The last request: nothing asked yet, in flight, answered, failed or not sendable. */
export type Status =
  | { readonly state: "idle" | "planning" | "planned" }
  | { readonly state: "failed"; readonly failure: Failure }
  | ({ readonly state: "incomplete" } & Problem)

/** The loops on screen; they stay usable when a later request fails. */
export type Shown = {
  readonly plan: LoopPlan
  readonly lines: ReadonlyArray<MapLine>
  readonly focused: number | undefined
}

export const LETTERS = ["A", "B", "C"]

export function alternativesMap(shown: Shown, index: number): MapRoutes {
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
export type SelectionStatus =
  | { readonly state: "idle" | "saving" | "saved" | "checking" }
  | Extract<RouteSelectionRead, { state: "failed" }>

export function ActiveSelection({
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

export function StatusMessage({
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

export function LoopAlternatives({
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
              <span className="loop-alternative-mark" aria-hidden="true" />
              <span className="loop-alternative-name">
                Loop {LETTERS[index] ?? index + 1}
              </span>
              <span className="loop-alternative-distance">
                {formatKilometers(route.distance_meters)}
              </span>
              <span className="loop-alternative-fit" aria-hidden="true">
                {route.within_tolerance ? (
                  <CheckIcon size={20} />
                ) : (
                  <ApproximateEqualsIcon size={20} />
                )}
              </span>
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
