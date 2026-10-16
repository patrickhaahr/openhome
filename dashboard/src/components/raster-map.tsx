import { useEffect, useRef, useState } from "react"
import type * as Leaflet from "leaflet"
import type {
  LatLngBoundsExpression,
  LayerGroup,
  Map as LeafletMap,
} from "leaflet"
import { CrosshairSimpleIcon } from "@phosphor-icons/react/dist/csr/CrosshairSimple"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Button } from "@/components/ui/button"
import {
  routePadding,
  type MapLine,
  type MapOrigin,
  type MapRoutes,
} from "@/lib/map"

/**
 * OpenStreetMap's own raster tiles: keyless under its tile policy for light use,
 * with visible attribution and the browser's Referer left intact.
 */
const TILE_URL = "https://tile.openstreetmap.org/{z}/{x}/{y}.png"

const ATTRIBUTION =
  '© <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'

const LOCAL_ZOOM = 14

/** A stalled tile server must not leave the view loading forever. */
const LOAD_TIMEOUT_MS = 20_000

/** Overlay colors from the dashboard theme the map is drawn under. */
export type RasterPalette = {
  readonly origin: string
  readonly casing: string
  readonly alternative: string
}

type RasterView =
  | { state: "loading" }
  | { state: "ready"; tiles: "complete" | "incomplete" }
  | { state: "provider-unavailable" | "failed" }

function prefersReducedMotion(): boolean {
  return matchMedia("(prefers-reduced-motion: reduce)").matches
}

/** Leaflet takes `[latitude, longitude]`; map lines are GeoJSON order. */
function latLngs(line: MapLine): Array<[number, number]> {
  return line.coordinates.map(([longitude, latitude]) => [latitude, longitude])
}

function routeBounds(lines: ReadonlyArray<MapLine>): LatLngBoundsExpression {
  return lines.flatMap(latLngs)
}

/**
 * The shared map for browsers without WebGL2, such as the photo frame's
 * Mali-400 WebView: OpenStreetMap image tiles under the same origin, routes and
 * controls, flat and north-up. Image tiles need no GPU parsing, which also
 * suits its 512 MB of memory. Leaflet loads after mount and is removed on unmount.
 */
export function RasterMap({
  origin,
  routes,
  palette,
  shade,
  credit,
}: {
  origin: MapOrigin
  routes: MapRoutes | undefined
  palette: RasterPalette
  /** Night darkens the light OSM tiles to sit in the night dashboard. */
  shade: "day" | "night"
  /** An extra credit merged into the map's one attribution. */
  credit?: string | undefined
}): React.JSX.Element {
  const container = useRef<HTMLDivElement>(null)
  const map = useRef<LeafletMap | undefined>(undefined)
  const overlays = useRef<LayerGroup | undefined>(undefined)
  const fittedLines = useRef<ReadonlyArray<MapLine> | undefined>(undefined)
  const [view, setView] = useState<RasterView>({ state: "loading" })
  const [attempt, setAttempt] = useState(0)
  const leaflet = useRef<typeof Leaflet | undefined>(undefined)
  // Counts created maps, so overlays are drawn again on each new one.
  const [created, setCreated] = useState(0)

  useEffect(() => {
    const element = container.current

    if (element === null) return

    setView({ state: "loading" })

    let instance: LeafletMap | undefined
    let disposed = false

    function fail(state: "provider-unavailable" | "failed"): void {
      if (disposed) return

      disposed = true
      clearTimeout(timeout)
      instance?.remove()
      instance = undefined
      map.current = undefined
      overlays.current = undefined
      setView({ state })
    }

    const timeout = setTimeout(() => {
      fail("provider-unavailable")
    }, LOAD_TIMEOUT_MS)

    import("leaflet").then(
      ({ default: L }) => {
        if (disposed) return

        const reduced = prefersReducedMotion()

        instance = L.map(element, {
          center: [origin.latitude, origin.longitude],
          zoom: LOCAL_ZOOM,
          minZoom: 10,
          maxZoom: 18,
          zoomControl: false,
          zoomAnimation: !reduced,
          fadeAnimation: !reduced,
          markerZoomAnimation: !reduced,
        })
        instance.attributionControl.setPrefix(false)

        if (credit !== undefined)
          instance.attributionControl.addAttribution(credit)

        const tiles = L.tileLayer(TILE_URL, {
          maxZoom: 18,
          attribution: ATTRIBUTION,
          // Keep fewer off-screen tiles decoded on a 512 MB frame.
          keepBuffer: 1,
          updateWhenZooming: false,
        })

        // Any tile proves the provider answers; later failures only thin the map.
        tiles.on("tileload", () => {
          clearTimeout(timeout)

          if (!disposed)
            setView((current) =>
              current.state === "loading"
                ? { state: "ready", tiles: "complete" }
                : current
            )
        })
        tiles.on("tileerror", () => {
          if (!disposed)
            setView((current) =>
              current.state === "ready"
                ? { ...current, tiles: "incomplete" }
                : current
            )
        })
        tiles.addTo(instance)

        leaflet.current = L
        map.current = instance
        overlays.current = L.layerGroup().addTo(instance)
        fittedLines.current = undefined
        setCreated((count) => count + 1)
      },
      () => {
        fail("failed")
      }
    )

    return () => {
      disposed = true
      clearTimeout(timeout)
      instance?.remove()
      map.current = undefined
      overlays.current = undefined
    }
  }, [origin.latitude, origin.longitude, attempt, credit])

  useEffect(() => {
    const current = map.current
    const group = overlays.current
    const L = leaflet.current

    if (current === undefined || group === undefined || L === undefined) return

    group.clearLayers()

    const lines = routes?.lines ?? []

    // Alternatives first, so the highlighted route is drawn above shared streets.
    const ordered = lines
      .map((line, index) => ({ line, focused: index === routes?.focused }))
      .sort((left, right) => Number(left.focused) - Number(right.focused))

    for (const { line, focused } of ordered)
      L.polyline(latLngs(line), {
        color: palette.casing,
        weight: focused ? 9 : 6,
        lineCap: "round",
        lineJoin: "round",
        interactive: false,
      }).addTo(group)

    for (const { line, focused } of ordered)
      L.polyline(latLngs(line), {
        color: focused ? palette.origin : palette.alternative,
        weight: focused ? 5 : 3,
        opacity: focused ? 1 : 0.75,
        lineCap: "round",
        lineJoin: "round",
        interactive: false,
      }).addTo(group)

    const center: [number, number] = [origin.latitude, origin.longitude]

    L.circleMarker(center, {
      radius: 18,
      stroke: false,
      fillColor: palette.origin,
      fillOpacity: 0.18,
      interactive: false,
    }).addTo(group)
    L.circleMarker(center, {
      radius: 7,
      color: palette.casing,
      weight: 3,
      opacity: 1,
      fillColor: palette.origin,
      fillOpacity: 1,
      interactive: false,
    }).addTo(group)

    // Highlighting another alternative keeps the viewport the household chose.
    if (
      routes === undefined ||
      lines.length === 0 ||
      fittedLines.current === routes.lines
    )
      return

    fittedLines.current = routes.lines

    const { top, right, bottom, left } = routePadding(
      current.getContainer().clientWidth,
      current.getContainer().clientHeight
    )

    current.fitBounds(routeBounds(routes.lines), {
      paddingTopLeft: [left, top],
      paddingBottomRight: [right, bottom],
      maxZoom: 16,
      animate: !prefersReducedMotion(),
    })
  }, [routes, palette, origin.latitude, origin.longitude, created])

  function reload(): void {
    setView({ state: "loading" })
    setAttempt((count) => count + 1)
  }

  function zoom(by: 1 | -1): void {
    const current = map.current

    current?.setZoom(current.getZoom() + by, {
      animate: !prefersReducedMotion(),
    })
  }

  function recenter(): void {
    map.current?.setView([origin.latitude, origin.longitude], LOCAL_ZOOM, {
      animate: !prefersReducedMotion(),
    })
  }

  return (
    <div
      className="shared-map"
      data-map-state={view.state}
      data-map-renderer="raster"
      data-map-shade={shade}
    >
      <div
        ref={container}
        className="shared-map-canvas"
        aria-label={
          origin.source === "demo"
            ? "Map of the Copenhagen demo area"
            : "Map of the local area"
        }
      />
      {view.state === "ready" ? (
        <div className="shared-map-controls">
          <Button
            size="icon-lg"
            variant="secondary"
            aria-label="Zoom in"
            onClick={() => zoom(1)}
          >
            <PlusIcon size={22} aria-hidden="true" />
          </Button>
          <Button
            size="icon-lg"
            variant="secondary"
            aria-label="Zoom out"
            onClick={() => zoom(-1)}
          >
            <MinusIcon size={22} aria-hidden="true" />
          </Button>
          <Button
            size="icon-lg"
            variant="secondary"
            aria-label="Return to origin"
            onClick={recenter}
          >
            <CrosshairSimpleIcon size={22} aria-hidden="true" />
          </Button>
        </div>
      ) : null}
      <RasterStatus view={view} reload={reload} />
    </div>
  )
}

function RasterStatus({
  view,
  reload,
}: {
  view: RasterView
  reload: () => void
}): React.JSX.Element | null {
  switch (view.state) {
    case "loading":
      return (
        <p className="shared-map-message" role="status">
          Loading map…
        </p>
      )
    case "ready":
      return view.tiles === "complete" ? null : (
        <div className="shared-map-notices" role="status">
          <div className="shared-map-notice">
            <p>Some map tiles could not load. The map may be incomplete.</p>
            <Button variant="outline" onClick={reload}>
              Reload map
            </Button>
          </div>
        </div>
      )
    case "provider-unavailable":
      return (
        <RasterFailure
          title="Map tiles unavailable"
          detail="OpenStreetMap could not be reached. The rest of the dashboard still works."
          reload={reload}
        />
      )
    case "failed":
      return (
        <RasterFailure
          title="Map unavailable"
          detail="The map could not start. Check the dashboard connection and try again."
          reload={reload}
        />
      )
  }
}

function RasterFailure({
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
