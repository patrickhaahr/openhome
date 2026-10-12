import { z } from "zod"
import type { MapRoutes } from "./map"

const activity = z.enum(["walking", "running"])

const distanceTarget = z.object({
  kind: z.literal("distance"),
  meters: z.number().int().positive(),
})

/** A request for loops from the shared map origin; Axum owns the supported ranges. */
export const loopRequest = z.object({
  activity,
  target: z.discriminatedUnion("kind", [
    distanceTarget,
    z.object({ kind: z.literal("steps"), steps: z.number().int().positive() }),
  ]),
  step_length_cm: z.number().int().positive(),
})

/** A request for walking or running loops by distance or steps. */
export type LoopRequest = z.infer<typeof loopRequest>

/** Walking or running. */
export type Activity = z.infer<typeof activity>

const coordinate = z.tuple([
  z.number().min(-180).max(180),
  z.number().min(-90).max(90),
])

const generatedLoop = z.object({
  id: z.uuid(),
  distance_meters: z.number().positive(),
  deviation_meters: z.number(),
  deviation_percent: z.number(),
  within_tolerance: z.boolean(),
  estimated_steps: z.number().int().nonnegative(),
  geometry: z.object({
    type: z.literal("LineString"),
    coordinates: z.array(coordinate).min(2),
  }),
})

const loopPlan = z.object({
  activity,
  target: z.discriminatedUnion("kind", [
    distanceTarget,
    z.object({
      kind: z.literal("steps"),
      steps: z.number().int().positive(),
      meters: z.number().int().positive(),
    }),
  ]),
  step_length_cm: z.number().int().positive(),
  tolerance_percent: z.number().positive(),
  sought: z.number().int().positive(),
  search: z.enum(["complete", "exhausted", "interrupted"]),
  loops: z.array(generatedLoop).min(1),
})

/** Axum's loop generation contract, stripping unknown fields. */
export const loopRead = z.discriminatedUnion("state", [
  z.object({ state: z.literal("generated"), plan: loopPlan }),
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

/** Axum's explanation for a request outside the supported ranges. */
export const loopRejection = z.object({ error: z.string().min(1).max(200) })

/** Distinct measured loops for one request, matching loops first. */
export type LoopPlan = z.infer<typeof loopPlan>

/** A measured loop whose distance describes its drawn geometry. */
export type GeneratedLoop = LoopPlan["loops"][number]

/**
 * Generated loops, a provider outcome, a target Axum rejected, or a
 * dashboard-side connection failure.
 */
export type LoopRead =
  | z.infer<typeof loopRead>
  | { state: "rejected"; message: string }
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

const wholeNumber = new Intl.NumberFormat("en-GB", {
  maximumFractionDigits: 0,
})

/** Kilometres with two decimals, the precision a walker can act on. */
export function formatKilometers(meters: number): string {
  return `${(meters / 1000).toFixed(2)} km`
}

/** A step count with thousands separators. */
export function formatSteps(steps: number): string {
  return `${wholeNumber.format(steps)} steps`
}

/** The requested effort as the household asked for it. */
export function describeTarget(target: LoopPlan["target"]): string {
  switch (target.kind) {
    case "distance":
      return formatKilometers(target.meters)
    case "steps":
      return `${formatSteps(target.steps)} (${formatKilometers(target.meters)})`
  }
}

/** The plan's heading, such as "5.00 km walking loops". */
export function describePlan(plan: LoopPlan): string {
  return `${describeTarget(plan.target)} ${plan.activity} loops`
}

/** How a measured loop compares with its target, without calling a deviation a match. */
export function describeDeviation({
  route,
  plan,
}: {
  route: GeneratedLoop
  plan: LoopPlan
}): string {
  const target = formatKilometers(plan.target.meters)
  const percent = Math.abs(route.deviation_percent).toFixed(1)

  if (route.deviation_meters === 0) return `Exactly the ${target} target`

  const direction = route.deviation_meters > 0 ? "longer" : "shorter"
  const amount = `${Math.abs(route.deviation_meters)} m (${percent}%) ${direction} than ${target}`

  return route.within_tolerance
    ? `${amount}, within ±${plan.tolerance_percent}%`
    : `${amount}, outside ±${plan.tolerance_percent}%`
}

/** The stride-based step estimate, labelled as one. */
export function describeEstimatedSteps({
  route,
  plan,
}: {
  route: GeneratedLoop
  plan: LoopPlan
}): string {
  return `About ${formatSteps(route.estimated_steps)}, estimated from a ${plan.step_length_cm} cm step`
}

/**
 * Why fewer matching loops are shown than the search looked for, or
 * `undefined` when enough matched.
 */
export function describeShortfall(plan: LoopPlan): string | undefined {
  const matching = plan.loops.filter((route) => route.within_tolerance).length

  if (matching >= plan.sought) return undefined

  const tolerance = `within ±${plan.tolerance_percent}% of the target`

  const found =
    matching === 0
      ? `No loop came ${tolerance}`
      : `Only ${describeCount(matching, "distinct loop")} came ${tolerance}`

  const rest =
    plan.loops.length > matching
      ? " The closest others are shown with their deviation."
      : ""

  // A complete search found enough matches, so only these two can fall short.
  return plan.search === "interrupted"
    ? `The route planner stopped before the search finished. ${found}.${rest}`
    : `The search used all its attempts. ${found}.${rest}`
}

/** A count with its noun, such as "1 loop" or "3 loops". */
export function describeCount(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`
}

/** A short heading and the next step for a missing loop. */
export type LoopFailureCopy = {
  readonly title: string
  readonly detail: string
}

/** What the household can do about a failed request. */
export function describeFailure(
  read: Exclude<LoopRead, { state: "generated" }>
): LoopFailureCopy {
  if (read.state === "rejected")
    return { title: "Target not accepted", detail: read.message }

  switch (read.reason) {
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
          "Connect OpenHome to the household’s openrouteservice instance to plan loops.",
      }
    case "provider-unavailable":
      return {
        title: "Route planner unavailable",
        detail:
          "The household route planner could not plan loops right now. Try again shortly.",
      }
    case "no-route":
      return {
        title: "No loop found",
        detail:
          "The route planner found no loop from the origin. No route was invented.",
      }
    case "unsuitable-route":
      return {
        title: "No suitable loop",
        detail:
          "Every proposed loop started too far from the origin, did not close or was far from the target, so none is shown.",
      }
    case "connection-unavailable":
      return {
        title: "Route planning unavailable",
        detail: "OpenHome could not be reached. Check Connection or try again.",
      }
  }
}

/** A persisted route and its selection revision, ready for a phone to pin independently. */
export const activeRoute = generatedLoop.extend({
  version: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
  activity,
  target: loopPlan.shape.target,
  step_length_cm: z.number().int().positive(),
  tolerance_percent: z.number().positive(),
})

/** Axum's persisted selection, including a truthful absence. */
export const activeRouteRead = z.discriminatedUnion("state", [
  z.object({ state: z.literal("none") }),
  z.object({ state: z.literal("selected"), route: activeRoute }),
])

/** The requested selection references only a previously generated loop. */
export const routeSelection = z.object({ route_id: z.uuid() })

/** A selected route; the phone owns any started Route Session separately. */
export type ActiveRoute = z.infer<typeof activeRoute>

/** Failed reads cannot be mistaken for an empty selection. */
export type ActiveRouteRead =
  | z.infer<typeof activeRouteRead>
  | { state: "unavailable" }

/** Selection succeeds only after Axum confirms persistence. */
export type RouteSelectionRead =
  | Extract<ActiveRouteRead, { state: "selected" }>
  | {
      state: "failed"
      reason: "candidate-unavailable" | "connection-unavailable"
    }

/** Safe recovery state for failed reads or RPCs. */
export const unavailableActiveRoute: ActiveRouteRead = { state: "unavailable" }

/** Draw and name the confirmed selection independently of the compared alternatives. */
export function activeRouteMap(route: ActiveRoute): MapRoutes {
  return {
    lines: [route.geometry],
    focused: 0,
    label: `Showing active route · ${formatKilometers(route.distance_meters)}`,
  }
}
