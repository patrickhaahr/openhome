import type {
  FillExtrusionLayerSpecification,
  LngLatBoundsLike,
  Map as MapLibreMap,
} from "maplibre-gl"
import { type MapLine, type MapRoutes } from "@/lib/map"

/**
 * OpenFreeMap's free styles follow the dashboard's light: Positron by day, Dark at night.
 * Neither needs a key and both carry their own attribution. MapLibre cannot read CSS tokens,
 * so the dashboard's sun and ink are repeated here.
 */
export const mapThemes = {
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

export const LOCAL_ZOOM = 14

/** OpenMapTiles carries building heights from this zoom; lower zooms stay flat. */
const BUILDINGS_MIN_ZOOM = 14

/** Tilt for the 3D view: recognisable buildings without hiding the horizon-side streets. */
export const BUILDINGS_PITCH = 55

export const BUILDINGS_LAYER = "buildings-3d"

export const ROUTE_SOURCE = "route"

/** OpenFreeMap's vector source, which carries the OpenMapTiles building layer. */
export const BUILDINGS_SOURCE = "openmaptiles"

export const buildingsLayer = (theme: MapTheme) =>
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

export type MapView =
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
export type Perspective =
  | { mode: "2d"; reason: "chosen" | "graphics-failure" }
  | { mode: "3d" }

/** What the 3D view can show near the map center once rendering settles. */
export type BuildingCoverage = "shown" | "zoom-in" | "none-here"

/** Camera pitch and building visibility for a perspective. */
export function presentation(mode: Perspective["mode"]): {
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
export function supportsWebGL2(): boolean {
  const context = document.createElement("canvas").getContext("webgl2")

  context?.getExtension("WEBGL_lose_context")?.loseContext()

  return context !== null
}

export function prefersReducedMotion(): boolean {
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
export function routeFeatures(routes: MapRoutes | undefined): RouteData {
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
export function routeBounds(lines: ReadonlyArray<MapLine>): LngLatBoundsLike {
  const coordinates = lines.flatMap((line) => line.coordinates)
  const longitudes = coordinates.map(([longitude]) => longitude)
  const latitudes = coordinates.map(([, latitude]) => latitude)

  return [
    [Math.min(...longitudes), Math.min(...latitudes)],
    [Math.max(...longitudes), Math.max(...latitudes)],
  ]
}

/** Explains why 3D buildings are absent, or nothing when the view is as chosen. */
export function buildingsNotice({
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
export function buildingCoverage(map: MapLibreMap): BuildingCoverage {
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
