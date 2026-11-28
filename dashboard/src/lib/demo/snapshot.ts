import { z } from "zod"
import {
  json,
  shiftJson,
  shiftText,
  wholeDaysBetween,
  type Json,
} from "./date-shift"

export const snapshotFile = z.object({
  capturedOn: z.iso.date(),
  responses: z.record(z.string(), json),
})

export type Snapshot = z.infer<typeof snapshotFile>

export type DemoReply =
  | { readonly state: "found"; readonly body: Json }
  | { readonly state: "missing" }

export type DemoRequest = {
  readonly path: string
  readonly query: Readonly<Record<string, string>>
}

const progressPath = "/api/exercises/progress"

const progressBody = z.object({
  from: z.iso.date(),
  to: z.iso.date(),
  logged_exercise_count: z.number(),
  exercises: z.array(json),
})

const demoRoutes: ReadonlyArray<RegExp> = [
  /^\/api\/training(\/|$)/,
  /^\/api\/workouts(\/|$)/,
  /^\/api\/exercises(\/|$)/,
  /^\/api\/profile$/,
  /^\/api\/body_weight$/,
]

export function isDemoRoute(path: string): boolean {
  return demoRoutes.some((route) => route.test(path))
}

function mapSegments(
  path: string,
  change: (segment: string) => string
): string {
  return path
    .split("/")
    .map((segment) => encodeURIComponent(change(decodeURIComponent(segment))))
    .join("/")
}

export function requestKey(request: DemoRequest): string {
  const query = new URLSearchParams(
    Object.entries(request.query).sort(([a], [b]) => a.localeCompare(b))
  ).toString()

  return `${mapSegments(request.path, (segment) => segment)}?${query}`
}

function shiftRequest(request: DemoRequest, days: number): DemoRequest {
  return {
    path: mapSegments(request.path, (segment) => shiftText(segment, days)),
    query: Object.fromEntries(
      Object.entries(request.query).map(([name, value]) => [
        name,
        shiftText(value, days),
      ])
    ),
  }
}

function singleExercise(request: DemoRequest, id: string): DemoRequest {
  return { path: request.path, query: { ...request.query, exercise_ids: id } }
}

function composeProgress(
  snapshot: Snapshot,
  request: DemoRequest,
  ids: ReadonlyArray<string>
): Json | undefined {
  const pieces = ids.flatMap((id) => {
    const found = progressBody.safeParse(
      snapshot.responses[requestKey(singleExercise(request, id))]
    )

    return found.success ? [found.data] : []
  })

  const [first] = pieces

  return first === undefined || pieces.length < ids.length
    ? undefined
    : {
        ...first,
        exercises: pieces.flatMap((piece) => piece.exercises),
      }
}

function recordedBody(
  snapshot: Snapshot,
  request: DemoRequest
): Json | undefined {
  const direct = snapshot.responses[requestKey(request)]

  if (direct !== undefined) return direct

  const ids = request.query["exercise_ids"]?.split(",") ?? []

  return request.path === progressPath && ids.length > 1
    ? composeProgress(snapshot, request, ids)
    : undefined
}

export function answerFromSnapshot(
  snapshot: Snapshot,
  request: DemoRequest,
  today: string
): DemoReply {
  const days = wholeDaysBetween(snapshot.capturedOn, today)
  const body = recordedBody(snapshot, shiftRequest(request, -days))

  return body === undefined
    ? { state: "missing" }
    : { state: "found", body: shiftJson(body, days) }
}
