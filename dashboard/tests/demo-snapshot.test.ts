import { expect, test } from "vitest"
import {
  answerFromSnapshot,
  isDemoRoute,
  requestKey,
  type Snapshot,
} from "@/lib/demo/snapshot"

const strap = {
  source: "my-whoop",
  start: "2026-11-28T07:50:00+01:00",
  sport: "Calisthenics",
}

const detailPath = `/api/training/noop-workouts/${strap.source}/${encodeURIComponent(strap.start)}/${strap.sport}`

const line = (id: number) => ({
  exercise: { id, name: `Exercise ${id}`, category: "gym" },
  status: "never_logged",
})

const snapshot: Snapshot = {
  capturedOn: "2026-11-28",
  responses: {
    [requestKey({
      path: "/api/training/days/2026-11-28/recovery",
      query: {},
    })]: { day: "2026-11-28", start: "2026-11-27T23:25:00+01:00" },
    [requestKey({ path: detailPath, query: { route: "true" } })]: {
      start: strap.start,
    },
    [requestKey({
      path: "/api/exercises/progress",
      query: { from: "2026-10-31", to: "2026-11-28", exercise_ids: "7" },
    })]: {
      from: "2026-10-31",
      to: "2026-11-28",
      logged_exercise_count: 5,
      exercises: [line(7)],
    },
    [requestKey({
      path: "/api/exercises/progress",
      query: { from: "2026-10-31", to: "2026-11-28", exercise_ids: "9" },
    })]: {
      from: "2026-10-31",
      to: "2026-11-28",
      logged_exercise_count: 5,
      exercises: [line(9)],
    },
  },
}

test("a request made on the capture day is answered as recorded", () => {
  expect(
    answerFromSnapshot(
      snapshot,
      { path: "/api/training/days/2026-11-28/recovery", query: {} },
      "2026-11-28"
    )
  ).toEqual({
    state: "found",
    body: { day: "2026-11-28", start: "2026-11-27T23:25:00+01:00" },
  })
})

test("a later day finds the capture day's entry and receives it dated that day", () => {
  expect(
    answerFromSnapshot(
      snapshot,
      { path: "/api/training/days/2026-12-03/recovery", query: {} },
      "2026-12-03"
    )
  ).toEqual({
    state: "found",
    body: { day: "2026-12-03", start: "2026-12-02T23:25:00+01:00" },
  })
})

test("a datetime in the path is shifted back to find the recorded entry", () => {
  const moved = "2026-12-03T07:50:00+01:00"

  expect(
    answerFromSnapshot(
      snapshot,
      {
        path: `/api/training/noop-workouts/${strap.source}/${encodeURIComponent(moved)}/${strap.sport}`,
        query: { route: "true" },
      },
      "2026-12-03"
    )
  ).toEqual({ state: "found", body: { start: moved } })
})

test("a day outside the snapshot is missing", () => {
  expect(
    answerFromSnapshot(
      snapshot,
      { path: "/api/training/days/2026-06-01/recovery", query: {} },
      "2026-11-28"
    )
  ).toEqual({ state: "missing" })
})

test("a comparison of several exercises is assembled from the single ones in the order asked", () => {
  const reply = answerFromSnapshot(
    snapshot,
    {
      path: "/api/exercises/progress",
      query: { from: "2026-11-03", to: "2026-12-01", exercise_ids: "9,7" },
    },
    "2026-12-01"
  )

  expect(reply).toEqual({
    state: "found",
    body: {
      from: "2026-11-03",
      to: "2026-12-01",
      logged_exercise_count: 5,
      exercises: [line(9), line(7)],
    },
  })
})

test("a comparison naming an exercise that was not recorded is missing", () => {
  expect(
    answerFromSnapshot(
      snapshot,
      {
        path: "/api/exercises/progress",
        query: { from: "2026-10-31", to: "2026-11-28", exercise_ids: "7,8" },
      },
      "2026-11-28"
    )
  ).toEqual({ state: "missing" })
})

test("query order and path escaping do not change the key", () => {
  expect(requestKey({ path: "/api/x/a%3Ab", query: { b: "2", a: "1" } })).toBe(
    requestKey({ path: "/api/x/a:b", query: { a: "1", b: "2" } })
  )
})

test("only health and workout routes belong to demo data", () => {
  for (const path of [
    "/api/training/days/2026-11-28/recovery",
    "/api/workouts",
    "/api/workouts/12",
    "/api/exercises/progress",
    "/api/profile",
    "/api/body_weight",
  ])
    expect(isDemoRoute(path)).toBe(true)

  for (const path of [
    "/api/health",
    "/api/photos",
    "/api/profiles",
    "/api/nutrition/days",
  ])
    expect(isDemoRoute(path)).toBe(false)
})
