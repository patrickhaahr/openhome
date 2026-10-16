import { useEffect, useRef, useState } from "react"
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
import rasterMapCss from "leaflet/dist/leaflet.css?url"

/** Licence-required credit for the loops, merged into the map's own attribution. */
const ROUTES_CREDIT = "Routes by openrouteservice"

const travel = features[2] satisfies { id: "travel" }

/** Walking and running loops and, later, transit around the private origin. */
export const Route = createFileRoute("/travel")({
  head: () => ({
    meta: [{ title: `${travel.title} · OpenHome` }],
    links: [
      { rel: "stylesheet", href: mapCss },
      { rel: "stylesheet", href: rasterMapCss },
    ],
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
  const travelRef = useRef<HTMLElement>(null)

  useDockHeight(travelRef)

  const displayedRoutes =
    routes ??
    (active.state === "selected" ? activeRouteMap(active.route) : undefined)

  return (
    <section>
      <PageHead title={travel.title} />
      {read.state === "configured" ? (
        <figure className="travel" ref={travelRef}>
          <SharedMap
            key={`${read.origin.latitude},${read.origin.longitude}`}
            origin={read.origin}
            routes={displayedRoutes}
            theme={theme}
            credit={ROUTES_CREDIT}
          />
          {read.origin.source === "demo" ? (
            <p className="travel-demo">Demo origin</p>
          ) : null}
          <figcaption className="sr-only">
            {read.origin.source === "demo"
              ? "Demo origin · central Copenhagen"
              : "Private origin"}
            {" · "}
            {displayedRoutes?.label ??
              "Stops and live vehicles are not connected yet."}
          </figcaption>
          <LoopPlanner
            plan={requestLoops}
            initialActive={active}
            select={requestRouteSelection}
            reloadActive={loadActiveRoute}
            onRoutesChange={setRoutes}
          />
        </figure>
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

/**
 * Publishes the planner dock's height as `--dock-height`, so the map's attribution can sit
 * above a full-width dock on narrow screens instead of underneath it.
 */
function useDockHeight(travel: React.RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const element = travel.current
    const dock = element?.querySelector(".loop-dock")

    if (element === null || dock === null || dock === undefined) return

    if (typeof ResizeObserver === "undefined") return

    const observer = new ResizeObserver(() => {
      element.style.setProperty("--dock-height", `${dock.clientHeight}px`)
    })

    observer.observe(dock)

    return () => observer.disconnect()
  }, [travel])
}
