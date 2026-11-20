import { z } from "zod"

const origin = z.object({
  latitude: z.number().min(-85).max(85),
  longitude: z.number().min(-180).max(180),
  source: z.enum(["private", "demo"]),
})

export const mapOriginRead = z.discriminatedUnion("state", [
  z.object({ state: z.literal("configured"), origin }),
  z.object({
    state: z.literal("unavailable"),
    reason: z.literal("missing-configuration"),
  }),
])

export type MapOrigin = z.infer<typeof origin>

export type MapOriginRead =
  | z.infer<typeof mapOriginRead>
  | { state: "unavailable"; reason: "connection-unavailable" }

export const unavailableMapOrigin: MapOriginRead = {
  state: "unavailable",
  reason: "connection-unavailable",
}

export type MapLine = {
  readonly type: "LineString"
  readonly coordinates: ReadonlyArray<readonly [number, number]>
}

export type MapRoutes = {
  readonly label?: string
  readonly lines: ReadonlyArray<MapLine>
  readonly focused: number
  readonly endpoint?: readonly [number, number]
  readonly framing?: "even"
}

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

export const evenPadding = { top: 36, right: 36, bottom: 36, left: 36 }
