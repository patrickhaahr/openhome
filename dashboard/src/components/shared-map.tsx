import { useEffect, useRef, useState } from "react"
import type {
  FillExtrusionLayerSpecification,
  GeoJSONSource,
  LngLatBoundsLike,
  Map as MapLibreMap,
} from "maplibre-gl"
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url"
import { CrosshairSimpleIcon } from "@phosphor-icons/react/dist/csr/CrosshairSimple"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { RasterMap } from "@/components/raster-map"
import { Button } from "@/components/ui/button"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import {
  routePadding,
  type MapLine,
  type MapOrigin,
  type MapRoutes,
} from "@/lib/map"

/**
 * OpenFreeMap's free styles follow the dashboard's light: Positron by day, Dark at night.
 * Neither needs a key and both carry their own attribution. MapLibre cannot read CSS tokens,
 * so the dashboard's sun and ink are repeated here.
 */
const mapThemes = {
  day: {
    style: "https://tiles.openfreemap.org/styles/positron",
    origin: "#ff5b1f",
    casing: "#ffffff",
    alternative: "#5a5f69",
    buildings: "#d9d9d2",
  },
  night: {
    style: "https://tiles.openfreemap.org/styles/dark",
    origin: "#ff6a2e",
    casing: "#0b0c0e",
    alternative: "#9a9ea6",
    buildings: "#2a2c31",
  },
} as const

/** The map's palette, matching the dashboard theme it was created under. */
export type MapTheme = keyof typeof mapThemes

const LOCAL_ZOOM = 14

/** A stalled style or tile request must not leave the view loading forever. */
const LOAD_TIMEOUT_MS = 20_000

/** OpenMapTiles carries building heights from this zoom; lower zooms stay flat. */
const BUILDINGS_MIN_ZOOM = 14

/** Tilt for the 3D view: recognisable buildings without hiding the horizon-side streets. */
const BUILDINGS_PITCH = 55

const BUILDINGS_LAYER = "buildings-3d"

const ROUTE_SOURCE = "route"

/** OpenFreeMap's vector source, which carries the OpenMapTiles building layer. */
const BUILDINGS_SOURCE = "openmaptiles"

const buildingsLayer = (theme: MapTheme) =>
  ({
    id: BUILDINGS_LAYER,
    type: "fill-extrusion",
    source: BUILDINGS_SOURCE,
    "source-layer": "building",
    minzoom: BUILDINGS_MIN_ZOOM,
    filter: ["!=", ["get", "hide_3d"], true],
    paint: {
      "fill-extrusion-color": mapThemes[theme].buildings,
      "fill-extrusion-height": ["coalesce", ["get", "render_height"], 0],
      "fill-extrusion-base": ["coalesce", ["get", "render_min_height"], 0],
      "fill-extrusion-opacity": 0.9,
    },
  }) satisfies Omit<FillExtrusionLayerSpecification, "layout">

type MapView =
  | { state: "loading" }
  | {
      state: "ready"
      tiles: "complete" | "incomplete"
      buildings: "available" | "unavailable"
    }
  | { state: "provider-unavailable" | "graphics-unavailable" | "failed" }

/**
 * Flat north-up 2D, or pitched 3D with extruded buildings. A graphics failure
 * in 3D restarts the map flat and says why.
 */
type Perspective =
  | { mode: "2d"; reason: "chosen" | "graphics-failure" }
  | { mode: "3d" }

/** What the 3D view can show near the map center once rendering settles. */
type BuildingCoverage = "shown" | "zoom-in" | "none-here"

/** Camera pitch and building visibility for a perspective. */
function presentation(mode: Perspective["mode"]): {
  pitch: number
  buildings: "visible" | "none"
} {
  return mode === "3d"
    ? { pitch: BUILDINGS_PITCH, buildings: "visible" }
    : { pitch: 0, buildings: "none" }
}

/**
 * Whether MapLibre can render here. The probe's context is released at once;
 * GPUs limited to OpenGL ES 2.0, like the photo frame's Mali-400, offer none.
 */
function supportsWebGL2(): boolean {
  const context = document.createElement("canvas").getContext("webgl2")

  context?.getExtension("WEBGL_lose_context")?.loseContext()

  return context !== null
}

function prefersReducedMotion(): boolean {
  return matchMedia("(prefers-reduced-motion: reduce)").matches
}

/** GeoJSON for the route source: one line per alternative, or nothing. */
type RouteData = {
  type: "FeatureCollection"
  features: Array<{
    type: "Feature"
    properties: { focused: boolean }
    geometry: { type: "LineString"; coordinates: Array<[number, number]> }
  }>
}

/** Routes as GeoJSON for the route source; no routes draw nothing. */
function routeFeatures(routes: MapRoutes | undefined): RouteData {
  const features = (routes?.lines ?? []).map((line, index) => ({
    type: "Feature" as const,
    properties: { focused: index === routes?.focused },
    geometry: {
      type: "LineString" as const,
      coordinates: line.coordinates.map(
        ([longitude, latitude]): [number, number] => [longitude, latitude]
      ),
    },
  }))

  return { type: "FeatureCollection", features }
}

/** The south-west and north-east corners enclosing every route. */
function routeBounds(lines: ReadonlyArray<MapLine>): LngLatBoundsLike {
  const coordinates = lines.flatMap((line) => line.coordinates)
  const longitudes = coordinates.map(([longitude]) => longitude)
  const latitudes = coordinates.map(([, latitude]) => latitude)

  return [
    [Math.min(...longitudes), Math.min(...latitudes)],
    [Math.max(...longitudes), Math.max(...latitudes)],
  ]
}

/**
 * Interactive map centered on the private origin, switchable between flat 2D
 * and pitched 3D buildings. Switching keeps the map, its overlays and viewport.
 * Supplied routes are drawn above the buildings, the focused one highlighted,
 * and fitted into view once per set of alternatives.
 * MapLibre loads only after mount and is removed on unmount, releasing its
 * WebGL context when the view closes. Browsers without WebGL2 get the flat
 * image-tile `RasterMap` instead.
 */
export function SharedMap({
  origin,
  routes,
  theme = "night",
  credit,
}: {
  origin: MapOrigin
  routes?: MapRoutes | undefined
  /** Recreates the map in the other palette when the dashboard turns. */
  theme?: MapTheme
  /** An extra credit merged into the map's one attribution. */
  credit?: string | undefined
}): React.JSX.Element {
  const container = useRef<HTMLDivElement>(null)
  const map = useRef<MapLibreMap | undefined>(undefined)
  const [view, setView] = useState<MapView>({ state: "loading" })
  const [attempt, setAttempt] = useState(0)
  const [visible, setVisible] = useState(true)
  // Without WebGL2 the image-tile map stands in, and MapLibre is never loaded.
  const [renderer, setRenderer] = useState<"vector" | "raster">("vector")

  const [perspective, setPerspective] = useState<Perspective>({
    mode: "2d",
    reason: "chosen",
  })

  const [coverage, setCoverage] = useState<BuildingCoverage>("shown")
  // A retried map starts in the current perspective without recreating on toggles.
  const mode = useRef<Perspective["mode"]>("2d")

  // The viewport a graphics restart resumes from, consumed by the next map.
  const resume = useRef<{ center: [number, number]; zoom: number } | undefined>(
    undefined
  )

  // A recreated map draws the current routes without refitting the viewport to them.
  const shownRoutes = useRef<MapRoutes | undefined>(routes)
  const fittedLines = useRef<ReadonlyArray<MapLine> | undefined>(undefined)

  const center: [number, number] = [origin.longitude, origin.latitude]

  useEffect(() => {
    function visibilityChanged(): void {
      setVisible(!document.hidden)
    }

    visibilityChanged()
    document.addEventListener("visibilitychange", visibilityChanged)

    return () =>
      document.removeEventListener("visibilitychange", visibilityChanged)
  }, [])

  useEffect(() => {
    const element = container.current

    if (element === null || !visible || document.hidden) return

    if (!supportsWebGL2()) {
      // eslint-disable-next-line react/set-state-in-effect -- WebGL2 can only be probed in the browser after hydration; SSR has no canvas.
      setRenderer("raster")

      return
    }

    setView({ state: "loading" })
    // A recreated map measures its own buildings; never show the last map's notice.
    setCoverage("shown")

    let instance: MapLibreMap | undefined
    let disposed = false

    function fail(state: Exclude<MapView["state"], "loading" | "ready">): void {
      if (disposed) return

      // Cancel the pending import as well as the map: a late import must not
      // resurrect a view that already timed out or lost its graphics context.
      disposed = true
      clearTimeout(timeout)
      const failed = instance

      instance = undefined
      map.current = undefined
      failed?.remove()
      setView({ state })
    }

    const timeout = setTimeout(() => {
      fail("provider-unavailable")
    }, LOAD_TIMEOUT_MS)

    import("maplibre-gl").then(
      ({ Map, setWorkerUrl }) => {
        if (disposed) return

        try {
          // Vite moves the library into a chunk; its relative worker URL then
          // points at a missing file. Bundle the worker explicitly for both builds.
          setWorkerUrl(workerUrl)
          instance = new Map({
            container: element,
            style: mapThemes[theme].style,
            center: resume.current?.center ?? [
              origin.longitude,
              origin.latitude,
            ],
            zoom: resume.current?.zoom ?? LOCAL_ZOOM,
            minZoom: 10,
            maxZoom: 18,
            pitch: presentation(mode.current).pitch,
            // Only the perspective toggle tilts the map; pitch gestures stay off.
            maxPitch: BUILDINGS_PITCH,
            dragRotate: false,
            pitchWithRotate: false,
            touchPitch: false,
            attributionControl: {
              compact: false,
              customAttribution: credit === undefined ? [] : [credit],
            },
            fadeDuration: prefersReducedMotion() ? 0 : 300,
          })
        } catch {
          // MapLibre throws when WebGL2 cannot be created.
          fail("graphics-unavailable")

          return
        }

        const created = instance

        resume.current = undefined
        let loaded = false

        map.current = created
        created.touchZoomRotate.disableRotation()
        created.keyboard.disableRotation()

        created.on("load", () => {
          loaded = true
          clearTimeout(timeout)

          // A style without the building source still serves a usable 2D map.
          const buildings =
            created.getSource(BUILDINGS_SOURCE) === undefined
              ? "unavailable"
              : "available"

          if (buildings === "available") {
            // Below the labels, so street names stay readable over rooftops.
            const firstLabel = created
              .getStyle()
              .layers.find((layer) => layer.type === "symbol")?.id

            created.addLayer(
              {
                ...buildingsLayer(theme),
                layout: { visibility: presentation(mode.current).buildings },
              },
              firstLabel
            )
          }

          // Below the origin, so the start stays visible where the loop returns.
          created.addSource(ROUTE_SOURCE, {
            type: "geojson",
            data: routeFeatures(shownRoutes.current),
          })
          // Every casing sits below every line, and the sort key keeps the
          // highlighted route on top where loops share streets.
          created.addLayer({
            id: "route-casing",
            type: "line",
            source: ROUTE_SOURCE,
            layout: {
              "line-join": "round",
              "line-cap": "round",
              "line-sort-key": ["case", ["get", "focused"], 1, 0],
            },
            paint: {
              "line-color": mapThemes[theme].casing,
              "line-width": ["case", ["get", "focused"], 9, 6],
            },
          })
          created.addLayer({
            id: "route",
            type: "line",
            source: ROUTE_SOURCE,
            layout: {
              "line-join": "round",
              "line-cap": "round",
              "line-sort-key": ["case", ["get", "focused"], 1, 0],
            },
            paint: {
              "line-color": [
                "case",
                ["get", "focused"],
                mapThemes[theme].origin,
                mapThemes[theme].alternative,
              ],
              "line-width": ["case", ["get", "focused"], 5, 3],
              "line-opacity": ["case", ["get", "focused"], 1, 0.75],
            },
          })

          created.addSource("origin", {
            type: "geojson",
            data: {
              type: "Feature",
              properties: {},
              geometry: {
                type: "Point",
                coordinates: [origin.longitude, origin.latitude],
              },
            },
          })
          created.addLayer({
            id: "origin-halo",
            type: "circle",
            source: "origin",
            paint: {
              "circle-radius": 18,
              "circle-color": mapThemes[theme].origin,
              "circle-opacity": 0.18,
            },
          })
          created.addLayer({
            id: "origin",
            type: "circle",
            source: "origin",
            paint: {
              "circle-radius": 7,
              "circle-color": mapThemes[theme].origin,
              "circle-stroke-width": 3,
              "circle-stroke-color": mapThemes[theme].casing,
            },
          })

          if (!disposed)
            setView({ state: "ready", tiles: "complete", buildings })
        })
        created.on("idle", () => {
          if (disposed || mode.current !== "3d") return

          setCoverage(buildingCoverage(created))
        })
        created.on("error", () => {
          // Before load the style or source failed; afterwards only some tiles did.
          if (!loaded) {
            fail("provider-unavailable")

            return
          }

          if (!disposed)
            setView((current) =>
              current.state === "ready"
                ? { ...current, tiles: "incomplete" }
                : current
            )
        })
        created.on("webglcontextlost", () => {
          if (disposed) return

          if (mode.current !== "3d") {
            fail("graphics-unavailable")

            return
          }

          // Extrusions can exhaust older GPUs; restart flat rather than blank.
          disposed = true
          clearTimeout(timeout)
          resume.current = {
            center: created.getCenter().toArray(),
            zoom: created.getZoom(),
          }
          choose({ mode: "2d", reason: "graphics-failure" })
          setAttempt((count) => count + 1)
        })
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
    }
  }, [origin.latitude, origin.longitude, attempt, visible, theme, credit])

  const ready = view.state === "ready"

  useEffect(() => {
    const current = map.current

    if (!ready || current === undefined) return

    const { pitch, buildings } = presentation(perspective.mode)

    // Visibility is a layout change, so MapLibre re-parses the visible tiles on
    // each switch; a hidden layer keeps 2D from paying for extrusions (ADR 0007).
    if (
      current.getLayer(BUILDINGS_LAYER) !== undefined &&
      current.getLayoutProperty(BUILDINGS_LAYER, "visibility") !== buildings
    )
      current.setLayoutProperty(BUILDINGS_LAYER, "visibility", buildings)

    // A freshly loaded map already has this pitch; easing would cancel a pan.
    if (current.getPitch() !== pitch)
      current.easeTo({
        pitch,
        duration: prefersReducedMotion() ? 0 : 500,
      })
  }, [perspective.mode, ready])

  useEffect(() => {
    shownRoutes.current = routes
    const current = map.current

    if (!ready || current === undefined) return

    // MapLibre applies the data on its worker; the fit below does not depend on it.
    void current
      .getSource<GeoJSONSource>(ROUTE_SOURCE)
      ?.setData(routeFeatures(routes))

    // Highlighting another alternative keeps the viewport the household chose.
    if (
      routes === undefined ||
      routes.lines.length === 0 ||
      fittedLines.current === routes.lines
    )
      return

    fittedLines.current = routes.lines
    current.fitBounds(routeBounds(routes.lines), {
      padding: routePadding(
        current.getContainer().clientWidth,
        current.getContainer().clientHeight
      ),
      maxZoom: 16,
      pitch: current.getPitch(),
      duration: prefersReducedMotion() ? 0 : 600,
    })
  }, [routes, ready])

  function choose(next: Perspective): void {
    mode.current = next.mode

    // Coverage is measured again once the tilted map settles.
    if (next.mode === "3d") setCoverage("shown")
    setPerspective(next)
  }

  function reload(): void {
    setView({ state: "loading" })
    setAttempt((count) => count + 1)
  }

  function zoom(by: 1 | -1): void {
    map.current?.easeTo({
      zoom: map.current.getZoom() + by,
      duration: prefersReducedMotion() ? 0 : 250,
    })
  }

  function recenter(): void {
    map.current?.easeTo({
      center,
      zoom: LOCAL_ZOOM,
      duration: prefersReducedMotion() ? 0 : 400,
    })
  }

  if (renderer === "raster")
    return (
      <RasterMap
        origin={origin}
        routes={routes}
        palette={mapThemes[theme]}
        shade={theme}
        credit={credit}
      />
    )

  return (
    <div className="shared-map" data-map-state={view.state}>
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
          <div className="shared-map-perspective">
            <ToggleGroup
              aria-label="Map perspective"
              orientation="vertical"
              spacing={0}
              value={[perspective.mode]}
              onValueChange={([next]) => {
                if (next === "2d") choose({ mode: "2d", reason: "chosen" })

                if (next === "3d") choose({ mode: "3d" })
              }}
            >
              <ToggleGroupItem value="2d" size="lg">
                2D
              </ToggleGroupItem>
              <ToggleGroupItem
                value="3d"
                size="lg"
                aria-label="3D buildings"
                disabled={view.buildings === "unavailable"}
              >
                3D
              </ToggleGroupItem>
            </ToggleGroup>
          </div>
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
      <MapStatus
        view={view}
        perspective={perspective}
        coverage={coverage}
        reload={reload}
      />
    </div>
  )
}

function MapStatus({
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

/** Explains why 3D buildings are absent, or nothing when the view is as chosen. */
function buildingsNotice({
  view,
  perspective,
  coverage,
}: {
  view: Extract<MapView, { state: "ready" }>
  perspective: Perspective
  coverage: BuildingCoverage
}): string | undefined {
  if (view.buildings === "unavailable")
    return "3D buildings are unavailable from the map provider. The 2D map still works."

  if (perspective.mode === "2d")
    return perspective.reason === "graphics-failure"
      ? "3D buildings stopped after a graphics failure, so the map returned to 2D."
      : undefined

  switch (coverage) {
    case "shown":
      return undefined
    case "zoom-in":
      return "Zoom in to see buildings in 3D."
    case "none-here":
      // Failed tiles are already reported; they are not proof of no buildings.
      return view.tiles === "incomplete"
        ? undefined
        : "No building data near the map center. The map is tilted without buildings."
  }
}

/**
 * Whether the settled 3D view has extruded buildings near its center. Querying
 * only the central quarter keeps this cheap after every gesture on an older iPad.
 */
function buildingCoverage(map: MapLibreMap): BuildingCoverage {
  if (map.getZoom() < BUILDINGS_MIN_ZOOM) return "zoom-in"

  const { clientWidth: width, clientHeight: height } = map.getContainer()

  const buildings = map.queryRenderedFeatures(
    [
      [width / 4, height / 4],
      [(width * 3) / 4, (height * 3) / 4],
    ],
    { layers: [BUILDINGS_LAYER] }
  )

  return buildings.length > 0 ? "shown" : "none-here"
}
