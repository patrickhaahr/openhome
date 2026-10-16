import { useState } from "react"
import { createFileRoute } from "@tanstack/react-router"
import { MapTrifoldIcon } from "@phosphor-icons/react/dist/csr/MapTrifold"
import { CheckAgainButton } from "@/components/check-again-button"
import { SharedMap } from "@/components/shared-map"
import { LoopPlanner } from "@/components/loop-planner"
import { useMapTheme } from "@/components/daylight"
import { PageHead } from "@/components/page-head"
import { features } from "@/lib/features"
import {
  unavailableMapOrigin,
  type MapOriginRead,
  type MapRoutes,
} from "@/lib/map"
import { activeRouteMap, type ActiveRouteRead } from "@/lib/route-loop"
import { getMapOrigin } from "@/lib/map.functions"
import {
  loadActiveRoute,
  requestRouteSelection,
  requestLoops,
} from "@/lib/route-loop.functions"

import mapCss from "maplibre-gl/dist/maplibre-gl.css?url"

const travel = features[2] satisfies { id: "travel" }

/** Walking and running loops and, later, transit around the private origin. */
export const Route = createFileRoute("/travel")({
  head: () => ({
    meta: [{ title: `${travel.title} · OpenHome` }],
    links: [{ rel: "stylesheet", href: mapCss }],
  }),
  loader: async () => {
    const [map, active] = await Promise.all([
      getMapOrigin().catch(() => unavailableMapOrigin),
      loadActiveRoute(),
    ])

    return { map, active }
  },
  headers: () => ({ "Cache-Control": "no-store" }),
  component: TravelPage,
})

function TravelPage(): React.JSX.Element {
  const { map, active } = Route.useLoaderData()

  return (
    <TravelContent
      key={
        active.state === "selected"
          ? `${active.route.id}:${active.route.version}`
          : active.state
      }
      map={map}
      active={active}
    />
  )
}

function TravelContent({
  map: read,
  active,
}: {
  map: MapOriginRead
  active: ActiveRouteRead
}): React.JSX.Element {
  const [routes, setRoutes] = useState<MapRoutes | undefined>(undefined)
  const theme = useMapTheme()

  const displayedRoutes =
    routes ??
    (active.state === "selected" ? activeRouteMap(active.route) : undefined)

  return (
    <section>
      <PageHead title={travel.title} />
      {read.state === "configured" ? (
        <div className="travel-layout">
          <figure className="travel-map">
            <SharedMap
              key={`${read.origin.latitude},${read.origin.longitude}`}
              origin={read.origin}
              routes={displayedRoutes}
              theme={theme}
            />
            <figcaption>
              <span className="travel-map-origin">
                <span aria-hidden="true" />
                {read.origin.source === "demo"
                  ? "Demo origin · central Copenhagen"
                  : "Private origin"}
              </span>
              <span className="sr-only">
                {displayedRoutes?.label ??
                  "Stops and live vehicles are not connected yet."}
              </span>
            </figcaption>
          </figure>
          <LoopPlanner
            plan={requestLoops}
            initialActive={active}
            select={requestRouteSelection}
            reloadActive={loadActiveRoute}
            onRoutesChange={setRoutes}
          />
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
    <div className="state-note" data-failed="true">
      <MapTrifoldIcon size={56} weight="light" aria-hidden="true" />
      <h2>
        {reason === "missing-configuration"
          ? "Map setup needed"
          : "Map unavailable"}
      </h2>
      <p role="status" className="sr-only">
        {reason === "missing-configuration"
          ? "Set the private map origin on OpenHome to show the local area. There is no default location."
          : "OpenHome could not provide the map origin. Check Connection or try again."}
      </p>
      <CheckAgainButton compact />
    </div>
  )
}
