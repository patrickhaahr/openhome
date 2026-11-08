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
  evenPadding,
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

/** A stalled style, tile server or import must not leave either map loading forever. */
const LOAD_TIMEOUT_MS = 20_000

type AttemptResource<Resource extends { remove(): void }> =
  | { phase: "importing" }
  | { phase: "starting"; map: Resource }
  | { phase: "running"; map: Resource }
  | { phase: "released" }

/** One map attempt's resource, as seen by the library callbacks that outlive it. */
export type MapAttempt<Resource extends { remove(): void }> = {
  phase(): AttemptResource<Resource>["phase"]
  /** Takes a created map; one made after release is removed at once. */
  adopt(map: Resource): void
  /** First proof of life: stops the timeout. False for a released or repeated call. */
  settle(): boolean
  /** Stops the timeout and removes the map. Safe to repeat. */
  release(): void
}

/**
 * The owner of one attempt to bring up a map: the pending import, its load
 * timeout and the map it yields. It advances importing, starting, running and
 * released only forwards, so a late import, tile, style or graphics event that
 * finds the attempt released does nothing. Both maps share it.
 */
export function startMapAttempt<Resource extends { remove(): void }>({
  onTimeout,
  onMap,
}: {
  onTimeout: () => void
  /** Publishes the adopted map, then `undefined` when it is released. */
  onMap: (map: Resource | undefined) => void
}): MapAttempt<Resource> {
  let resource: AttemptResource<Resource> = { phase: "importing" }
  const timer = setTimeout(onTimeout, LOAD_TIMEOUT_MS)

  return {
    phase: () => resource.phase,
    adopt(map) {
      if (resource.phase !== "importing") {
        map.remove()

        return
      }

      resource = { phase: "starting", map }
      onMap(map)
    },
    settle() {
      if (resource.phase !== "starting") return false

      clearTimeout(timer)
      resource = { phase: "running", map: resource.map }

      return true
    },
    release() {
      const released = resource

      if (released.phase === "released") return

      resource = { phase: "released" }
      clearTimeout(timer)

      if (released.phase === "importing") return

      onMap(undefined)
      released.map.remove()
    },
  }
}

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
  controls = true,
}: {
  origin: MapOrigin
  routes: MapRoutes | undefined
  palette: RasterPalette
  /** Night darkens the light OSM tiles to sit in the night dashboard. */
  shade: "day" | "night"
  /** An extra credit merged into the map's one attribution. */
  credit?: string | undefined
  /** The zoom and recentre plate; a map that only shows a route can go without. */
  controls?: boolean
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

    const owner = startMapAttempt<LeafletMap>({
      onTimeout: () => fail("provider-unavailable"),
      onMap: (next) => {
        map.current = next

        if (next === undefined) overlays.current = undefined
      },
    })

    function fail(state: "provider-unavailable" | "failed"): void {
      if (owner.phase() === "released") return

      owner.release()
      setView({ state })
    }

    import("leaflet").then(
      ({ default: L }) => {
        if (owner.phase() === "released") return

        const reduced = prefersReducedMotion()

        const instance = L.map(element, {
          center: [origin.latitude, origin.longitude],
          zoom: LOCAL_ZOOM,
          minZoom: 10,
          maxZoom: 18,
          zoomControl: false,
          zoomAnimation: !reduced,
          fadeAnimation: !reduced,
          markerZoomAnimation: !reduced,
        })

        owner.adopt(instance)

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
          if (owner.settle()) setView({ state: "ready", tiles: "complete" })
        })
        tiles.on("tileerror", () => {
          if (owner.phase() === "running")
            setView((current) =>
              current.state === "ready"
                ? { ...current, tiles: "incomplete" }
                : current
            )
        })
        tiles.addTo(instance)

        leaflet.current = L
        overlays.current = L.layerGroup().addTo(instance)
        fittedLines.current = undefined
        setCreated((count) => count + 1)
      },
      () => {
        fail("failed")
      }
    )

    return () => {
      owner.release()
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

    if (routes?.endpoint !== undefined)
      L.circleMarker([routes.endpoint[1], routes.endpoint[0]], {
        radius: 12,
        color: palette.origin,
        weight: 3,
        opacity: 1,
        fillColor: palette.casing,
        fillOpacity: 1,
        interactive: false,
      }).addTo(group)

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

    const { top, right, bottom, left } =
      routes.framing === "even"
        ? evenPadding
        : routePadding(
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
      {view.state === "ready" && controls ? (
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
