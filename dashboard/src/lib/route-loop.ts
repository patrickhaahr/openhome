import { z } from "zod"

/** The loop the Travel view offers until distance and step targets arrive. */
export const walkingLoopRequest = z.object({
  activity: z.literal("walking"),
  target_meters: z.literal(5000),
})

/** A request for a walking loop from the shared map origin. */
export type WalkingLoopRequest = z.infer<typeof walkingLoopRequest>

const coordinate = z.tuple([
  z.number().min(-180).max(180),
  z.number().min(-90).max(90),
])

const walkingLoop = z.object({
  activity: z.literal("walking"),
  target_meters: z.number().int().positive(),
  distance_meters: z.number().positive(),
  deviation_meters: z.number(),
  deviation_percent: z.number(),
  tolerance_percent: z.number().positive(),
  within_tolerance: z.boolean(),
  geometry: z.object({
    type: z.literal("LineString"),
    coordinates: z.array(coordinate).min(2),
  }),
})

/** Axum's loop generation contract, stripping unknown fields. */
export const loopRead = z.discriminatedUnion("state", [
  z.object({ state: z.literal("generated"), route: walkingLoop }),
  z.object({
    state: z.literal("unavailable"),
    reason: z.enum([
      "missing-origin",
      "missing-provider",
      "provider-unavailable",
      "no-route",
      "unsuitable-route",
    ]),
  }),
])

/** A measured loop whose distance describes its drawn geometry. */
export type WalkingLoop = z.infer<typeof walkingLoop>

/** A generated loop, a provider outcome, or a dashboard-side connection failure. */
export type LoopRead =
  | z.infer<typeof loopRead>
  | { state: "unavailable"; reason: "connection-unavailable" }

/** Why no loop is shown. */
export type LoopUnavailable = Extract<
  LoopRead,
  { state: "unavailable" }
>["reason"]

/** A failed transport or RPC never becomes a fabricated route. */
export const unavailableLoop: LoopRead = {
  state: "unavailable",
  reason: "connection-unavailable",
}

/** Kilometres with two decimals, the precision a walker can act on. */
export function formatKilometers(meters: number): string {
  return `${(meters / 1000).toFixed(2)} km`
}

/** How the measured loop compares with its target, without calling a deviation a match. */
export function describeDeviation(route: WalkingLoop): string {
  const target = formatKilometers(route.target_meters)
  const percent = Math.abs(route.deviation_percent).toFixed(1)

  if (route.deviation_meters === 0) return `Exactly the ${target} target`

  const direction = route.deviation_meters > 0 ? "longer" : "shorter"
  const amount = `${Math.abs(route.deviation_meters)} m (${percent}%) ${direction} than the ${target} target`

  return route.within_tolerance
    ? `${amount}, within ±${route.tolerance_percent}%`
    : `${amount}, outside ±${route.tolerance_percent}%`
}

/** A short heading and the next step for a missing loop. */
export type LoopUnavailableCopy = {
  readonly title: string
  readonly detail: string
}

/** What the household can do about a missing loop. */
export function describeUnavailable(
  reason: LoopUnavailable
): LoopUnavailableCopy {
  switch (reason) {
    case "missing-origin":
      return {
        title: "Map origin needed",
        detail:
          "Loops start at the private map origin. Set it on OpenHome first.",
      }
    case "missing-provider":
      return {
        title: "Route planning not set up",
        detail:
          "Connect OpenHome to the household’s openrouteservice instance to plan walking loops.",
      }
    case "provider-unavailable":
      return {
        title: "Route planner unavailable",
        detail:
          "The household route planner could not plan a loop right now. Try again shortly.",
      }
    case "no-route":
      return {
        title: "No walkable loop found",
        detail:
          "The route planner found no walking loop from the origin. No route was invented.",
      }
    case "unsuitable-route":
      return {
        title: "No suitable loop",
        detail:
          "The proposed loop started too far from the origin, did not close or was far from the target distance, so it is not shown.",
      }
    case "connection-unavailable":
      return {
        title: "Route planning unavailable",
        detail: "OpenHome could not be reached. Check Connection or try again.",
      }
  }
}
