import type {
  FillExtrusionLayerSpecification,
  LngLatBoundsLike,
  Map as MapLibreMap,
} from "maplibre-gl"
import { type MapLine, type MapRoutes } from "@/lib/map"

export const mapStyles = {
  day: "https://tiles.openfreemap.org/styles/positron",
  night: "https://tiles.openfreemap.org/styles/dark",
} as const

export type MapTheme = keyof typeof mapStyles

export type MapPalette = {
  readonly origin: string
  readonly casing: string
  readonly alternative: string
  readonly buildings: string
}

const paletteTokens = {
  origin: "--sun",
  casing: "--map-casing",
  alternative: "--muted-ink",
  buildings: "--map-buildings",
} as const satisfies Record<keyof MapPalette, string>

const probeAttribute = {
  day: { name: "data-theme", value: "light" },
  night: { name: "class", value: "dark" },
} as const satisfies Record<MapTheme, { name: string; value: string }>

export function mapPalette(theme: MapTheme): MapPalette {
  const probe = document.createElement("div")
  const { name, value } = probeAttribute[theme]
  probe.setAttribute(name, value)
  document.documentElement.append(probe)

  const style = getComputedStyle(probe)

  const colour = (token: string): string => style.getPropertyValue(token).trim()

  const palette = {
    origin: colour(paletteTokens.origin),
    casing: colour(paletteTokens.casing),
    alternative: colour(paletteTokens.alternative),
    buildings: colour(paletteTokens.buildings),
  }

  probe.remove()

  return palette
}

export const LOCAL_ZOOM = 14

const BUILDINGS_MIN_ZOOM = 14

export const BUILDINGS_PITCH = 55

export const BUILDINGS_LAYER = "buildings-3d"

export const ROUTE_SOURCE = "route"

export const BUILDINGS_SOURCE = "openmaptiles"

export const buildingsLayer = (buildings: string) =>
  ({
    id: BUILDINGS_LAYER,
    type: "fill-extrusion",
    source: BUILDINGS_SOURCE,
    "source-layer": "building",
    minzoom: BUILDINGS_MIN_ZOOM,
    filter: ["!=", ["get", "hide_3d"], true],
    paint: {
      "fill-extrusion-color": buildings,
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

export type Perspective =
  | { mode: "2d"; reason: "chosen" | "graphics-failure" }
  | { mode: "3d" }

export type BuildingCoverage = "shown" | "zoom-in" | "none-here"

export function presentation(mode: Perspective["mode"]): {
  pitch: number
  buildings: "visible" | "none"
} {
  return mode === "3d"
    ? { pitch: BUILDINGS_PITCH, buildings: "visible" }
    : { pitch: 0, buildings: "none" }
}

export function supportsWebGL2(): boolean {
  const context = document.createElement("canvas").getContext("webgl2")

  context?.getExtension("WEBGL_lose_context")?.loseContext()

  return context !== null
}

export function prefersReducedMotion(): boolean {
  return matchMedia("(prefers-reduced-motion: reduce)").matches
}

type RouteData = {
  type: "FeatureCollection"
  features: Array<{
    type: "Feature"
    properties: { focused: boolean }
    geometry: { type: "LineString"; coordinates: Array<[number, number]> }
  }>
}

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

export function routeBounds(lines: ReadonlyArray<MapLine>): LngLatBoundsLike {
  const coordinates = lines.flatMap((line) => line.coordinates)
  const longitudes = coordinates.map(([longitude]) => longitude)
  const latitudes = coordinates.map(([, latitude]) => latitude)

  return [
    [Math.min(...longitudes), Math.min(...latitudes)],
    [Math.max(...longitudes), Math.max(...latitudes)],
  ]
}

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
      return view.tiles === "incomplete"
        ? undefined
        : "No building data near the map center. The map is tilted without buildings."
  }
}

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
