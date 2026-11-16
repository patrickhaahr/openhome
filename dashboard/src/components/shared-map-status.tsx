import { Button } from "@/components/ui/button"
import {
  type MapView,
  type Perspective,
  type BuildingCoverage,
  buildingsNotice,
} from "@/components/shared-map-config"

export function MapStatus({
  view,
  perspective,
  coverage,
  reload,
}: {
  view: MapView
  perspective: Perspective
  coverage: BuildingCoverage
  reload: () => void
}): React.JSX.Element | null {
  switch (view.state) {
    case "loading":
      return (
        <p className="shared-map-message" role="status">
          Loading map…
        </p>
      )
    case "ready": {
      const buildings = buildingsNotice({ view, perspective, coverage })

      if (view.tiles === "complete" && buildings === undefined) return null

      return (
        <div className="shared-map-notices" role="status">
          {view.tiles === "incomplete" ? (
            <div className="shared-map-notice">
              <p>Some map tiles could not load. The map may be incomplete.</p>
              <Button variant="outline" onClick={reload}>
                Reload map
              </Button>
            </div>
          ) : null}
          {buildings === undefined ? null : (
            <p className="shared-map-notice">{buildings}</p>
          )}
        </div>
      )
    }

    case "provider-unavailable":
      return (
        <MapFailure
          title="Map tiles unavailable"
          detail="OpenFreeMap could not be reached. The rest of the dashboard still works."
          reload={reload}
        />
      )
    case "graphics-unavailable":
      return (
        <MapFailure
          title="Map graphics unavailable"
          detail="This browser could not start WebGL2 map rendering. The rest of the dashboard still works."
          reload={reload}
        />
      )
    case "failed":
      return (
        <MapFailure
          title="Map unavailable"
          detail="The map could not start. Check the dashboard connection and try again."
          reload={reload}
        />
      )
  }
}

function MapFailure({
  title,
  detail,
  reload,
}: {
  title: string
  detail: string
  reload: () => void
}): React.JSX.Element {
  return (
    <div className="shared-map-message" role="status">
      <h3>{title}</h3>
      <p>{detail}</p>
      <Button size="lg" variant="outline" onClick={reload}>
        Try again
      </Button>
    </div>
  )
}
