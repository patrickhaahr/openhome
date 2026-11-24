import { useEffect, useRef, useState } from "react"
import type * as Leaflet from "leaflet"
import type {
  LatLngBoundsExpression,
  LayerGroup,
  Map as LeafletMap,
} from "leaflet"
import { MapControls } from "@/components/shared-map-controls"
import { MapFrame } from "@/components/shared-map-frame"
import {
  MapFailure,
  MapLoading,
  MapNotices,
  TilesIncomplete,
} from "@/components/shared-map-status"
import { mapPalette } from "@/components/shared-map-config"
import {
  evenPadding,
  routePadding,
  type MapLine,
  type MapOrigin,
  type MapRoutes,
} from "@/lib/map"

const TILE_URL = "https://tile.openstreetmap.org/{z}/{x}/{y}.png"

const ATTRIBUTION =
  '© <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'

const LOCAL_ZOOM = 14

const LOAD_TIMEOUT_MS = 20_000

type AttemptResource<Resource extends { remove(): void }> =
  | { phase: "importing" }
  | { phase: "starting"; map: Resource }
  | { phase: "running"; map: Resource }
  | { phase: "released" }

export type MapAttempt<Resource extends { remove(): void }> = {
  phase(): AttemptResource<Resource>["phase"]
  adopt(map: Resource): void
  settle(): boolean
  release(): void
}

export function startMapAttempt<Resource extends { remove(): void }>({
  onTimeout,
  onMap,
}: {
  onTimeout: () => void
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

type RasterView =
  | { state: "loading" }
  | { state: "ready"; tiles: "complete" | "incomplete" }
  | { state: "provider-unavailable" | "failed" }

function prefersReducedMotion(): boolean {
  return matchMedia("(prefers-reduced-motion: reduce)").matches
}

function latLngs(line: MapLine): Array<[number, number]> {
  return line.coordinates.map(([longitude, latitude]) => [latitude, longitude])
}

function routeBounds(lines: ReadonlyArray<MapLine>): LatLngBoundsExpression {
  return lines.flatMap(latLngs)
}

export function RasterMap({
  origin,
  routes,
  shade,
  credit,
  controls = true,
}: {
  origin: MapOrigin
  routes: MapRoutes | undefined
  shade: "day" | "night"
  credit?: string | undefined
  controls?: boolean
}): React.JSX.Element {
  const container = useRef<HTMLDivElement>(null)
  const map = useRef<LeafletMap | undefined>(undefined)
  const overlays = useRef<LayerGroup | undefined>(undefined)
  const fittedLines = useRef<ReadonlyArray<MapLine> | undefined>(undefined)
  const [view, setView] = useState<RasterView>({ state: "loading" })
  const [attempt, setAttempt] = useState(0)
  const leaflet = useRef<typeof Leaflet | undefined>(undefined)
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
          keepBuffer: 1,
          updateWhenZooming: false,
        })

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

    const palette = mapPalette(shade)

    group.clearLayers()

    const lines = routes?.lines ?? []

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
  }, [routes, shade, origin.latitude, origin.longitude, created])

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
    <MapFrame
      state={view.state}
      renderer="raster"
      shade={shade}
      canvas={container}
      source={origin.source}
    >
      {view.state === "ready" && controls ? (
        <MapControls zoom={zoom} recenter={recenter} />
      ) : null}
      <RasterStatus view={view} reload={reload} />
    </MapFrame>
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
      return <MapLoading />
    case "ready":
      return view.tiles === "complete" ? null : (
        <MapNotices>
          <TilesIncomplete reload={reload} />
        </MapNotices>
      )
    case "provider-unavailable":
      return (
        <MapFailure
          title="Map tiles unavailable"
          detail="OpenStreetMap could not be reached. The rest of the dashboard still works."
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
