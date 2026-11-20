// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}

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

function titles(): Array<string | null> {
  return Array.from(
    container.querySelectorAll(
      '[data-slot="session"]:not([data-kind="strap"]) [data-slot="session-title"] h3'
    )
  ).map((title) => title.textContent)
}

function allTitles(): Array<string | null> {
  return Array.from(
    container.querySelectorAll('[data-slot="session-title"] h3')
  ).map((title) => title.textContent)
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
  expect(container.textContent).toContain("7 reps in 40 s · +12.35 kg")
  expect(container.textContent).toContain("Hold 1:05 · bodyweight · RPE 6")

  const underlines = Array.from(
    container.querySelectorAll<HTMLElement>('[data-slot="set-pill-rpe"]')
  ).map((line) => line.style.getPropertyValue("--rpe"))

  expect(underlines).toEqual(["0.8", "0.6"])
  expect(
    Array.from(container.querySelectorAll('[data-slot="set-pill"]')).map(
      (pill) => pill.getAttribute("data-rpe")
    )
  ).toEqual(["8", null, null, "6"])

  expect(
    container
      .querySelector<HTMLElement>('[data-slot="session-exercises"]')
      ?.style.getPropertyValue("--set-columns")
  ).toBe("3")
  expect(
    container.querySelector('[data-slot="session-counts"]')?.textContent
  ).toBe("Exercises2 exercisesSets4 sets")
})

test("the Log offers no way to create, edit or delete a workout", async () => {
  await store("2026-10-05", "Only session")
  await render()

  expect(titles()).toEqual(["Only session"])
  expect(container.querySelector('[data-slot="session"] button')).toBeNull()
  expect(container.querySelector('[data-slot="session"] a')).toBeNull()
  expect(container.querySelector("form")).toBeNull()
  expect(container.querySelector('[data-slot="set-pill-rpe"]')).toBeNull()
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
  expect(allTitles()).toHaveLength(4)
  expect(allTitles()[0]).toBe("Mobility day")
  expect(
    [...allTitles()].sort((a, b) => (a ?? "").localeCompare(b ?? ""))
  ).toEqual(["Cycling", "Mobility day", "Quiet day", "Run / walk"])

  const plates = Array.from(
    container.querySelectorAll('[data-slot="session"]')
  ).map((plate) => [
    plate.getAttribute("data-kind"),
    plate.querySelector("h3")?.textContent,
  ])

  expect(plates).toContainEqual(["paired", "Mobility day"])
  expect(plates).toContainEqual(["strap", "Cycling"])
  expect(plates).toContainEqual(["logged", "Quiet day"])
  expect(container.querySelector('[data-slot="session-map"]')).toBeNull()

  const paired = container.querySelector(
    '[data-slot="session"][data-kind="paired"]'
  )

  expect(paired?.querySelector('[data-slot="session-logged"]')).not.toBeNull()
  expect(paired?.querySelector('[data-slot="session-strap"]')).not.toBeNull()
  expect(
    container.querySelector('[data-slot="session"] a[href*="/recovery"]')
  ).toBeNull()
})
