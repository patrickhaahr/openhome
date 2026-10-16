import { z } from "zod"

const origin = z.object({
  latitude: z.number().min(-85).max(85),
  longitude: z.number().min(-180).max(180),
  source: z.enum(["private", "demo"]),
})

/** Axum's map origin contract, stripping unknown fields. */
export const mapOriginRead = z.discriminatedUnion("state", [
  z.object({ state: z.literal("configured"), origin }),
  z.object({
    state: z.literal("unavailable"),
    reason: z.literal("missing-configuration"),
  }),
])

/** Where shared maps open; `demo` is an unrelated public Copenhagen point. */
export type MapOrigin = z.infer<typeof origin>

/** A configured origin, missing setup, or a dashboard-side connection failure. */
export type MapOriginRead =
  | z.infer<typeof mapOriginRead>
  | { state: "unavailable"; reason: "connection-unavailable" }

/** A failed transport or RPC never falls back to a default location. */
export const unavailableMapOrigin: MapOriginRead = {
  state: "unavailable",
  reason: "connection-unavailable",
}

/** A line drawn on shared maps, as GeoJSON `[longitude, latitude]` pairs. */
export type MapLine = {
  readonly type: "LineString"
  readonly coordinates: ReadonlyArray<readonly [number, number]>
}

/** Alternative lines drawn together, with the one being inspected highlighted. */
export type MapRoutes = {
  /** The route currently displayed, named for the household beside the map. */
  readonly label?: string
  readonly lines: ReadonlyArray<MapLine>
  /** Index into `lines` of the highlighted line. */
  readonly focused: number
  /** Where the route ends, as `[longitude, latitude]`; drawn as a ring round the start. */
  readonly endpoint?: readonly [number, number]
  /** `even` frames the route with equal margins, for a map with no floating plates. */
  readonly framing?: "even"
}

/**
 * Room a fitted route keeps clear of the floating plates. Wide maps keep the planner's
 * results on the left and the controls on the right; narrow ones stack the planner below.
 */
export function routePadding(
  width: number,
  height: number
): { top: number; right: number; bottom: number; left: number } {
  return width > 900
    ? { top: 72, right: 112, bottom: 96, left: 416 }
    : {
        top: 72,
        right: 96,
        bottom: Math.min(440, Math.round(height * 0.55)),
        left: 40,
      }
}

/** Equal margins for a route framed on a map with no floating plates. */
export const evenPadding = { top: 36, right: 36, bottom: 36, left: 36 }
