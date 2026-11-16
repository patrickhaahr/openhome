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
      return <MapLoading />
    case "ready": {
      const buildings = buildingsNotice({ view, perspective, coverage })

      if (view.tiles === "complete" && buildings === undefined) return null

      return (
        <MapNotices>
          {view.tiles === "incomplete" ? (
            <TilesIncomplete reload={reload} />
          ) : null}
          {buildings === undefined ? null : (
            <p className={notice}>{buildings}</p>
          )}
        </MapNotices>
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

const message =
  "absolute inset-0 grid place-content-center justify-items-center gap-3 p-8 text-center leading-relaxed text-muted-foreground"

const notice =
  "pointer-events-auto flex w-fit flex-wrap items-center gap-x-4 gap-y-3 rounded-travel-plate bg-ground px-4 py-3 text-small"

export function MapLoading(): React.JSX.Element {
  return (
    <p className={message} role="status">
      Loading map…
    </p>
  )
}

/** Notices stack below the top edge, clear of the controls; only the notices take the pointer. */
export function MapNotices({
  children,
}: {
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <div
      className="pointer-events-none absolute top-travel-inset right-travel-inset left-travel-inset mt-12 mr-20 grid justify-items-start gap-2 max-phone:mt-20"
      role="status"
    >
      {children}
    </div>
  )
}

export function TilesIncomplete({
  reload,
}: {
  reload: () => void
}): React.JSX.Element {
  return (
    <div className={notice}>
      <p>Some map tiles could not load. The map may be incomplete.</p>
      <Button variant="outline" onClick={reload}>
        Reload map
      </Button>
    </div>
  )
}

export function MapFailure({
  title,
  detail,
  reload,
}: {
  title: string
  detail: string
  reload: () => void
}): React.JSX.Element {
  return (
    <div className={message} role="status">
      <h3 className="text-xlarge font-travel-heading text-foreground">
        {title}
      </h3>
      <p className="max-w-md">{detail}</p>
      <Button size="lg" variant="outline" onClick={reload}>
        Try again
      </Button>
    </div>
  )
}
