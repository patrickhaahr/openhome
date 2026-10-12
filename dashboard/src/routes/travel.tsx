import { useState } from "react"
import { createFileRoute } from "@tanstack/react-router"
import { MapTrifoldIcon } from "@phosphor-icons/react/dist/csr/MapTrifold"
import { CheckAgainButton } from "@/components/check-again-button"
import { SharedMap } from "@/components/shared-map"
import { LoopPlanner } from "@/components/loop-planner"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { features } from "@/lib/features"
import {
  unavailableMapOrigin,
  type MapOriginRead,
  type MapRoutes,
} from "@/lib/map"
import { getMapOrigin } from "@/lib/map.functions"
import { requestLoops } from "@/lib/route-loop.functions"

import mapCss from "maplibre-gl/dist/maplibre-gl.css?url"

const travel = features[2] satisfies { id: "travel" }

/** Walking and running loops and, later, transit around the private origin. */
export const Route = createFileRoute("/travel")({
  head: () => ({
    meta: [{ title: `${travel.title} · OpenHome` }],
    links: [{ rel: "stylesheet", href: mapCss }],
  }),
  loader: () => getMapOrigin().catch(() => unavailableMapOrigin),
  headers: () => ({ "Cache-Control": "no-store" }),
  component: TravelPage,
})

function TravelPage(): React.JSX.Element {
  const read = Route.useLoaderData()
  const [routes, setRoutes] = useState<MapRoutes | undefined>(undefined)

  return (
    <section className="focused-page travel-page">
      <header>
        <p className="page-eyebrow">OpenHome / {travel.label}</p>
        <h1 id="page-heading" tabIndex={-1}>
          {travel.title}
        </h1>
        <p className="feature-intro">
          Plan walking or running loops from the origin and explore the local
          area. Departures and live vehicles will appear on this map as they are
          added.
        </p>
      </header>
      {read.state === "configured" ? (
        <div className="travel-layout">
          <figure className="travel-map">
            <SharedMap
              key={`${read.origin.latitude},${read.origin.longitude}`}
              origin={read.origin}
              routes={routes}
            />
            <figcaption>
              <span className="travel-map-origin">
                <span aria-hidden="true" />
                {read.origin.source === "demo"
                  ? "Demo origin · central Copenhagen"
                  : "Private origin"}
              </span>
              <span>Stops and live vehicles are not connected yet.</span>
            </figcaption>
          </figure>
          <LoopPlanner plan={requestLoops} onRoutesChange={setRoutes} />
        </div>
      ) : (
        <MapSetup reason={read.reason} />
      )}
    </section>
  )
}

function MapSetup({
  reason,
}: {
  reason: Extract<MapOriginRead, { state: "unavailable" }>["reason"]
}): React.JSX.Element {
  return (
    <Card>
      <CardHeader>
        <MapTrifoldIcon
          size={40}
          aria-hidden="true"
          className="text-muted-foreground"
        />
        <h2>
          {reason === "missing-configuration"
            ? "Map setup needed"
            : "Map unavailable"}
        </h2>
      </CardHeader>
      <CardContent>
        <div className="travel-setup">
          <p role="status">
            {reason === "missing-configuration"
              ? "Set the private map origin on OpenHome to show the local area. There is no default location."
              : "OpenHome could not provide the map origin. Check Connection or try again."}
          </p>
          <CheckAgainButton />
        </div>
      </CardContent>
    </Card>
  )
}
