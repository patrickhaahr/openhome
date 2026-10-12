import { createFileRoute } from "@tanstack/react-router"
import { MapTrifoldIcon } from "@phosphor-icons/react/dist/csr/MapTrifold"
import { CheckAgainButton } from "@/components/check-again-button"
import { SharedMap } from "@/components/shared-map"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { features } from "@/lib/features"
import { unavailableMapOrigin, type MapOriginRead } from "@/lib/map"
import { getMapOrigin } from "@/lib/map.functions"

import mapCss from "maplibre-gl/dist/maplibre-gl.css?url"

const travel = features[2] satisfies { id: "travel" }

/** Routes and transit around the private origin; overlays arrive with their integrations. */
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

  return (
    <section className="focused-page travel-page">
      <header>
        <p className="page-eyebrow">OpenHome / {travel.label}</p>
        <h1 id="page-heading" tabIndex={-1}>
          {travel.title}
        </h1>
        <p className="feature-intro">
          Explore the local area. Walking routes, departures and live vehicles
          will appear on this map as they are added.
        </p>
      </header>
      {read.state === "configured" ? (
        <figure className="travel-map">
          <SharedMap
            key={`${read.origin.latitude},${read.origin.longitude}`}
            origin={read.origin}
          />
          <figcaption>
            <span className="travel-map-origin">
              <span aria-hidden="true" />
              {read.origin.source === "demo"
                ? "Demo origin · central Copenhagen"
                : "Private origin"}
            </span>
            <span>No routes, stops or vehicles are connected yet.</span>
          </figcaption>
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
