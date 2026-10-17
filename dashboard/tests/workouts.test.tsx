// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// This harness also executes the private server transport; CORS applies to browser RPCs, not Axum calls.

import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  test,
  vi,
} from "vitest"
import { z } from "zod"
import { Workouts } from "@/components/workouts"
import {
  exercise,
  workoutDetail,
  workoutSummary,
  type WorkoutSearch,
} from "@/lib/workout"
import { readWorkouts } from "@/lib/workout.server"

import { startAxumFixture } from "./support/axum-fixture"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let search: WorkoutSearch = {}

const auth = { Authorization: "Bearer synthetic-dashboard-api-key" }

beforeAll(async () => {
  fixture = await startAxumFixture()
})

afterAll(async () => {
  await fixture.stop()
})

beforeEach(() => {
  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
  search = {}
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.unstubAllGlobals()
  vi.unstubAllEnvs()

  // Clean through the public API; seeded library exercises remain.
  const workouts = await api(workoutRows, "/api/workouts?limit=500")

  for (const workout of workouts)
    await fetch(`${fixture.url}/api/workouts/${workout.id}`, {
      method: "DELETE",
      headers: auth,
    })
})

const workoutRows = workoutSummary.array()

async function api<T>(
  schema: z.ZodType<T>,
  path: string,
  init?: { method: "POST"; body: string }
): Promise<T> {
  const response = await fetch(`${fixture.url}${path}`, {
    method: init?.method ?? "GET",
    headers: { ...auth, "Content-Type": "application/json" },
    body: init?.body ?? null,
  })

  if (!response.ok) throw new Error(`${path} answered ${response.status}`)

  return schema.parse(await response.json())
}

async function libraryId(name: string): Promise<number> {
  const match = (await api(exercise.array(), "/api/exercises")).find(
    (item) => item.name === name
  )

  if (match === undefined) throw new Error(`Missing exercise: ${name}`)

  return match.id
}

type StoredSet = {
  readonly reps: number | null
  readonly weight_kg: number | null
  readonly duration_seconds: number | null
  readonly rpe: number | null
}

/** The web client never writes a Workout, so each one is stored through the API first. */
async function store(
  date: string,
  name: string,
  entries: ReadonlyArray<readonly [string, ReadonlyArray<StoredSet>]> = []
): Promise<void> {
  const exercises = await Promise.all(
    entries.map(async ([exerciseName, sets], order) => ({
      exercise_id: await libraryId(exerciseName),
      order_index: order,
      notes: null,
      sets: sets.map((set, index) => ({
        set_number: index + 1,
        notes: null,
        ...set,
      })),
    }))
  )

  await api(workoutDetail, "/api/workouts", {
    method: "POST",
    body: JSON.stringify({ date, name, notes: null, exercises }),
  })
}

async function render(): Promise<void> {
  const data = await readWorkouts(search)

  await act(async () => {
    root.render(
      <Workouts
        data={data}
        showOlder={async () => {
          search = { ...search, shown: (search.shown ?? 20) + 20 }
          await render()
        }}
      />
    )
  })
}

/** The names of the sessions that have a logged workout, in the order the Log lists them. */
function titles(): Array<string | null> {
  return Array.from(
    container.querySelectorAll(
      '.session:not([data-kind="strap"]) .session-title h3'
    )
  ).map((title) => title.textContent)
}

/** Every session's name, strap-only ones included. */
function allTitles(): Array<string | null> {
  return Array.from(container.querySelectorAll(".session-title h3")).map(
    (title) => title.textContent
  )
}

test("reviews reps, added weight, RPE and a bodyweight hold from history", async () => {
  await store("2026-10-10", "Pull day", [
    [
      "Pull-up",
      [
        { reps: 8, weight_kg: 20, duration_seconds: null, rpe: 8 },
        { reps: 6, weight_kg: 20, duration_seconds: null, rpe: null },
        { reps: 7, weight_kg: 12.345, duration_seconds: 40, rpe: null },
      ],
    ],
    [
      "Planche Hold",
      [{ reps: null, weight_kg: null, duration_seconds: 65, rpe: 6 }],
    ],
  ])
  await render()

  expect(titles()).toEqual(["Pull day"])
  expect(container.textContent).toContain("8 reps · +20 kg · RPE 8")
  expect(container.textContent).toContain("6 reps · +20 kg")
  // A duration stored on a reps set is shown, and a weight is never rounded away.
  expect(container.textContent).toContain("7 reps in 40 s · +12.35 kg")
  // Bodyweight reads as bodyweight, never as zero added weight.
  expect(container.textContent).toContain("Hold 1:05 · bodyweight · RPE 6")

  // RPE draws as an underline on the sets that have one, as long as the effort was hard.
  const underlines = Array.from(
    container.querySelectorAll<HTMLElement>(".set-pill-rpe")
  ).map((line) => line.style.getPropertyValue("--rpe"))

  expect(underlines).toEqual(["0.8", "0.6"])
  expect(
    Array.from(container.querySelectorAll(".set-pill")).map((pill) =>
      pill.getAttribute("data-rpe")
    )
  ).toEqual(["8", null, null, "6"])

  // Every exercise row shares one grid as wide as the longest exercise.
  expect(
    container
      .querySelector<HTMLElement>(".session-exercises")
      ?.style.getPropertyValue("--set-columns")
  ).toBe("3")
  expect(container.querySelector(".session-counts")?.textContent).toBe(
    "Exercises2 exercisesSets4 sets"
  )
})

test("the Log offers no way to create, edit or delete a workout", async () => {
  await store("2026-10-05", "Only session")
  await render()

  expect(titles()).toEqual(["Only session"])
  expect(container.querySelector(".session button")).toBeNull()
  expect(container.querySelector(".session a")).toBeNull()
  expect(container.querySelector("form")).toBeNull()
  expect(container.querySelector(".set-pill-rpe")).toBeNull()
})

test("an unreachable read is unavailable rather than an empty history", async () => {
  vi.stubEnv("OPENHOME_API_URL", "http://127.0.0.1:9")
  await render()
  expect(container.textContent).toContain("Workouts unavailable")
  expect(container.textContent).not.toContain("Ready when you are.")
})

test("a logged workout and the strap's session on its day share one plate; the strap's other sessions stand alone", async () => {
  await store("2026-09-30", "Mobility day")
  await store("2026-09-12", "Quiet day")

  await render()
  // Only the strap's Mobility session is not movement, so it is the one that pairs.
  expect(allTitles()).toHaveLength(4)
  expect(allTitles()[0]).toBe("Mobility day")
  expect(
    [...allTitles()].sort((a, b) => (a ?? "").localeCompare(b ?? ""))
  ).toEqual(["Cycling", "Mobility day", "Quiet day", "Run / walk"])

  const plates = Array.from(container.querySelectorAll(".session")).map(
    (plate) => [
      plate.getAttribute("data-kind"),
      plate.querySelector("h3")?.textContent,
    ]
  )

  expect(plates).toContainEqual(["paired", "Mobility day"])
  expect(plates).toContainEqual(["strap", "Cycling"])
  expect(plates).toContainEqual(["logged", "Quiet day"])
  // None of these sessions has a GPS route, so none draws a map or an empty frame for one.
  expect(container.querySelector(".session-map")).toBeNull()
  // The paired plate carries both halves, and nothing on it leads away to Recovery.
  const paired = container.querySelector('.session[data-kind="paired"]')

  expect(paired?.querySelector(".session-logged")).not.toBeNull()
  expect(paired?.querySelector(".session-strap")).not.toBeNull()
  expect(container.querySelector('.session a[href*="/recovery"]')).toBeNull()
})
