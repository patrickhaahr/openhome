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
}
