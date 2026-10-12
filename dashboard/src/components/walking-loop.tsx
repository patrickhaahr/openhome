import { useState } from "react"
import { PersonSimpleWalkIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleWalk"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import type { MapLine } from "@/lib/map"
import {
  describeDeviation,
  describeUnavailable,
  formatKilometers,
  type LoopRead,
  type LoopUnavailableCopy,
  type WalkingLoop,
  type WalkingLoopRequest,
} from "@/lib/route-loop"

const REQUEST: WalkingLoopRequest = { activity: "walking", target_meters: 5000 }

/** Planner state: nothing asked yet, a request in flight, or Axum's outcome. */
type Planning =
  | { state: "idle" }
  | { state: "planning" }
  | { state: "planned"; read: LoopRead }

/**
 * Plan a 5 km walking loop from the shared map origin and report its measured
 * length. The drawn route always comes from the provider's geometry.
 */
export function WalkingLoopPlanner({
  plan,
  onRouteChange,
}: {
  plan: (request: WalkingLoopRequest) => Promise<LoopRead>
  onRouteChange: (route: MapLine | undefined) => void
}): React.JSX.Element {
  const [planning, setPlanning] = useState<Planning>({ state: "idle" })

  async function planLoop(): Promise<void> {
    setPlanning({ state: "planning" })
    const read = await plan(REQUEST)

    setPlanning({ state: "planned", read })
    // A failed request clears the map: an old loop must not look like this answer.
    onRouteChange(read.state === "generated" ? read.route.geometry : undefined)
  }

  const busy = planning.state === "planning"

  return (
    <Card aria-busy={busy}>
      <CardHeader>
        <p className="walking-loop-eyebrow">
          <PersonSimpleWalkIcon size={20} aria-hidden="true" />
          Walking loop
        </p>
        <h2>5 km from the origin</h2>
      </CardHeader>
      <CardContent>
        <div className="walking-loop-body">
          <div role="status" className="walking-loop-result">
            <PlanningResult planning={planning} />
          </div>
          <Button
            size="lg"
            disabled={busy}
            onClick={() => {
              void planLoop()
            }}
          >
            {planButtonLabel(planning)}
          </Button>
          <p className="walking-loop-attribution">
            Routes © openrouteservice.org by HeiGIT · Map data © OpenStreetMap
            contributors
          </p>
        </div>
      </CardContent>
    </Card>
  )
}

function planButtonLabel(planning: Planning): string {
  switch (planning.state) {
    case "idle":
      return "Plan a 5 km walk"
    case "planning":
      return "Planning…"
    case "planned":
      return planning.read.state === "generated" ? "Plan again" : "Try again"
  }
}

function PlanningResult({
  planning,
}: {
  planning: Planning
}): React.JSX.Element {
  switch (planning.state) {
    case "idle":
      return (
        <p className="walking-loop-hint">
          A loop that returns to the origin, preferring parks and quiet paths
          where they are mapped.
        </p>
      )
    case "planning":
      return <p className="walking-loop-hint">Planning a walking loop…</p>
    case "planned":
      return planning.read.state === "generated" ? (
        <MeasuredLoop route={planning.read.route} />
      ) : (
        <LoopUnavailableNotice {...describeUnavailable(planning.read.reason)} />
      )
  }
}

function MeasuredLoop({ route }: { route: WalkingLoop }): React.JSX.Element {
  return (
    <>
      <p className="walking-loop-distance">
        <span>{formatKilometers(route.distance_meters)}</span>
        measured loop
      </p>
      <p
        className="walking-loop-fit"
        data-fit={route.within_tolerance ? "within" : "outside"}
      >
        {describeDeviation(route)}
      </p>
      <p className="walking-loop-hint">
        Shown on the map. It starts and ends at the origin.
      </p>
    </>
  )
}

function LoopUnavailableNotice({
  title,
  detail,
}: LoopUnavailableCopy): React.JSX.Element {
  return (
    <>
      <h3>{title}</h3>
      <p className="walking-loop-hint">{detail}</p>
    </>
  )
}
