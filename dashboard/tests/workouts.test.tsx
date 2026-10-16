// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// This harness also executes the private server transport; CORS applies to browser RPCs, not Axum calls.

import { act, useSyncExternalStore } from "react"
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
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router"
import { z } from "zod"
import { Workouts } from "@/components/workouts"
import {
  exercise,
  workoutDetail,
  workoutSummary,
  type WorkoutMutation,
  type WorkoutSearch,
} from "@/lib/workout"
import { readWorkouts, writeWorkout } from "@/lib/workout.server"

import { startAxumFixture } from "./support/axum-fixture"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let search: WorkoutSearch = {}

let mutationWork: Promise<WorkoutMutation | undefined> =
  Promise.resolve(undefined)

let refreshWork: Promise<void> = Promise.resolve()

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
  mounted = false
  search = {}
  mutationWork = Promise.resolve(undefined)
  refreshWork = Promise.resolve()
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

  for (const item of await api(exercise.array(), "/api/exercises"))
    if (item.name.startsWith("Synthetic "))
      await fetch(`${fixture.url}/api/exercises/${item.id}`, {
        method: "DELETE",
        headers: auth,
      })
})

const workoutRows = workoutSummary.array()

async function api<T>(
  schema: z.ZodType<T>,
  path: string,
  init?: { method: "POST" | "PATCH" | "DELETE"; body?: string }
): Promise<T> {
  const response = await fetch(`${fixture.url}${path}`, {
    method: init?.method ?? "GET",
    headers: { ...auth, "Content-Type": "application/json" },
    body: init?.body ?? null,
  })

  if (!response.ok) throw new Error(`${path} answered ${response.status}`)

  return schema.parse(
    response.status === 204 ? undefined : await response.json()
  )
}

async function libraryId(name: string): Promise<number> {
  const match = (await api(exercise.array(), "/api/exercises")).find(
    (item) => item.name === name
  )

  if (match === undefined) throw new Error(`Missing exercise: ${name}`)

  return match.id
}

async function stored(id: number): Promise<z.infer<typeof workoutDetail>> {
  return api(workoutDetail, `/api/workouts/${id}`)
}

/** Sessions link to Recovery, so the page sits in a router; renders swap the page without remounting it. */
let page: React.JSX.Element = <></>

let mounted = false

const listeners = new Set<() => void>()

function Page(): React.JSX.Element {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)

      return () => listeners.delete(listener)
    },
    () => page
  )
}

async function show(next: React.JSX.Element): Promise<void> {
  page = next

  await act(async () => {
    if (!mounted) {
      mounted = true

      const rootRoute = createRootRoute({ component: Page })

      const tree = rootRoute.addChildren([
        createRoute({ getParentRoute: () => rootRoute, path: "/recovery" }),
      ])

      const router = createRouter({
        routeTree: tree,
        history: createMemoryHistory({ initialEntries: ["/"] }),
      })

      await router.load()
      root.render(<RouterProvider router={router} />)
    }

    for (const listener of listeners) listener()
  })
}

async function render(): Promise<void> {
  const data = await readWorkouts(search)

  await show(
    <Workouts
      data={data}
      change={(command) => {
        const work = writeWorkout(command)
        mutationWork = work

        return work
      }}
      reload={() => {
        refreshWork = render()

        return refreshWork
      }}
      showOlder={() => {
        search = { ...search, shown: (search.shown ?? 20) + 20 }
        refreshWork = render()

        return refreshWork
      }}
    />
  )
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

function control<T extends HTMLElement>(label: string, kind: new () => T): T {
  const byAria = document.querySelector(`[aria-label="${label}"]`)

  const byLabel = Array.from(document.querySelectorAll("label")).find(
    (element) => element.textContent === label
  )

  const match = byAria ?? document.getElementById(byLabel?.htmlFor ?? "")

  if (!(match instanceof kind)) throw new Error(`Missing control: ${label}`)

  return match
}

function button(name: string, scope: ParentNode = document): HTMLButtonElement {
  const match = Array.from(scope.querySelectorAll("button")).find(
    (element) =>
      (element.getAttribute("aria-label") ?? element.textContent.trim()) ===
      name
  )

  if (match === undefined) throw new Error(`Missing button: ${name}`)

  return match
}

async function click(
  name: string,
  scope: ParentNode = document
): Promise<void> {
  await act(async () => {
    button(name, scope).click()
    await mutationWork
    await refreshWork
  })
}

async function type(label: string, value: string): Promise<void> {
  const input = control(label, HTMLInputElement)

  await act(async () => {
    // Native setter represents typing rather than replacing React's tracked value.
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value"
    )?.set?.call(input, value)
    input.dispatchEvent(new Event("input", { bubbles: true }))
  })
}

async function choose(label: string, value: string): Promise<void> {
  const select = control(label, HTMLSelectElement)

  await act(async () => {
    select.value = value
    select.dispatchEvent(new Event("change", { bubbles: true }))
  })
}

async function addExercise(name: string): Promise<void> {
  await click("Add exercise")
  await type("Search exercises", name)
  await click(`Add ${name}`)
}

/** The web client only edits, so a workout to edit is stored through the API first. */
async function storeEmpty(date: string, name: string): Promise<void> {
  await api(workoutDetail, "/api/workouts", {
    method: "POST",
    body: JSON.stringify({ date, name, notes: null, exercises: [] }),
  })
}

test("logs reps, added weight, RPE and a bodyweight hold, then reviews them from history", async () => {
  await storeEmpty("2026-10-10", "Pull day")
  await render()
  expect(titles()).toEqual(["Pull day"])
  await click("Edit Pull day")
  await addExercise("Pull-up")
  await type("Pull-up set 1 reps", "8")
  await type("Pull-up set 1 added weight in kg", "20")
  await choose("Pull-up set 1 RPE", "8")
  await click("Add set")
  // A new set repeats the previous one, so only the change needs typing.
  await type("Pull-up set 2 reps", "6")
  await addExercise("Planche Hold")
  await click("Hold", control("Planche Hold set 1 type", HTMLElement))
  await type("Planche Hold set 1 hold time", "1:05")
  await click("Save changes")

  expect(container.textContent).toContain("Workout saved to OpenHome.")

  const [summary] = await api(workoutRows, "/api/workouts")

  if (summary === undefined) throw new Error("Expected a stored workout")

  const workout = await stored(summary.id)

  expect(workout).toMatchObject({ date: "2026-10-10", name: "Pull day" })
  expect(
    workout.exercises.map((entry) => ({
      exercise: entry.exercise.name,
      sets: entry.sets.map(({ reps, weight_kg, duration_seconds, rpe }) => ({
        reps,
        weight_kg,
        duration_seconds,
        rpe,
      })),
    }))
  ).toEqual([
    {
      exercise: "Pull-up",
      sets: [
        { reps: 8, weight_kg: 20, duration_seconds: null, rpe: 8 },
        { reps: 6, weight_kg: 20, duration_seconds: null, rpe: 8 },
      ],
    },
    {
      // Bodyweight is stored as null added weight, never zero.
      exercise: "Planche Hold",
      sets: [{ reps: null, weight_kg: null, duration_seconds: 65, rpe: null }],
    },
  ])

  // A fresh read renders the stored workout as review history.
  await render()
  expect(titles()).toEqual(["Pull day"])
  expect(container.textContent).toContain("8 reps · +20 kg · RPE 8")
  expect(container.textContent).toContain("6 reps · +20 kg · RPE 8")
  expect(container.textContent).toContain("Hold 1:05 · bodyweight")
})

test("shows unusable set fields beside the set without saving anything", async () => {
  await storeEmpty("2026-10-10", "Dip day")
  await render()
  await click("Edit Dip day")
  await addExercise("Dip")
  await click("Save changes")
  expect(container.textContent).toContain("Enter whole reps above zero.")
  expect(
    control("Dip set 1 reps", HTMLInputElement).getAttribute("aria-invalid")
  ).toBe("true")

  await type("Dip set 1 reps", "10")
  await type("Dip set 1 added weight in kg", "0")
  await click("Save changes")
  expect(container.textContent).toContain(
    "Enter added weight in kg above zero, or leave it empty for bodyweight."
  )
  const [row] = await api(workoutRows, "/api/workouts")

  expect(row?.name).toBe("Dip day")
  expect((await stored(row?.id ?? 0)).exercises).toEqual([])
})

test("a rejected edit keeps the draft and the stored workout, and saves once corrected", async () => {
  const pullUp = await libraryId("Pull-up")

  const backDay = (reps: number): string =>
    JSON.stringify({
      date: "2026-10-08",
      name: "Back day",
      notes: null,
      exercises: [
        {
          exercise_id: pullUp,
          order_index: 0,
          notes: null,
          sets: [
            {
              set_number: 1,
              reps,
              weight_kg: 12.345,
              duration_seconds: 40,
              rpe: null,
              notes: "Strict form",
            },
          ],
        },
      ],
    })

  const created = await api(workoutDetail, "/api/workouts", {
    method: "POST",
    body: backDay(5),
  })

  const row = await api(exercise, "/api/exercises", {
    method: "POST",
    body: JSON.stringify({ name: "Synthetic Row", category: "gym" }),
  })

  await render()
  // Another device corrects the workout after this page read it.
  await api(workoutDetail, `/api/workouts/${created.id}`, {
    method: "PATCH",
    body: backDay(6),
  })
  await click("Edit Back day")
  expect(control("Pull-up set 1 reps", HTMLInputElement).value).toBe("6")
  await type("Pull-up set 1 reps", "7")
  await addExercise("Synthetic Row")
  await type("Synthetic Row set 1 reps", "12")
  // The library changes on another device before this edit is saved.
  await api(z.undefined(), `/api/exercises/${row.id}`, { method: "DELETE" })
  await click("Save changes")

  // The failure sits beside the save button rather than above a long editor.
  expect(container.querySelector('form [role="alert"]')?.textContent).toContain(
    "An exercise in this workout is no longer in the library."
  )
  expect(control("Pull-up set 1 reps", HTMLInputElement).value).toBe("7")
  expect((await stored(created.id)).exercises).toHaveLength(1)
  expect((await stored(created.id)).exercises[0]?.sets[0]?.reps).toBe(6)

  await click("Remove Synthetic Row from this workout")
  await click("Save changes")

  const saved = await stored(created.id)

  expect(saved.exercises.map((entry) => entry.exercise.name)).toEqual([
    "Pull-up",
  ])
  // Stored values the form does not edit survive the full replacement unrounded.
  expect(saved.exercises[0]?.sets[0]).toMatchObject({
    reps: 7,
    weight_kg: 12.345,
    duration_seconds: 40,
    notes: "Strict form",
  })
  expect(container.textContent).toContain("7 reps in 40 s · +12.35 kg")
})

test("the web client can edit a workout but neither create nor delete one", async () => {
  await storeEmpty("2026-10-05", "Only session")
  await render()

  const names = Array.from(document.querySelectorAll("button")).map(
    (element) =>
      element.getAttribute("aria-label") ?? element.textContent.trim()
  )

  expect(names).toContain("Edit Only session")
  expect(names.some((name) => /^Log workout$|^Remove /.test(name))).toBe(false)
  expect(document.querySelector('[role="alertdialog"]')).toBeNull()
  expect(container.querySelector(".set-pill-rpe")).toBeNull()
})

test("an unreachable read is unavailable rather than an empty history", async () => {
  vi.stubEnv("OPENHOME_API_URL", "http://127.0.0.1:9")
  await render()
  expect(container.textContent).toContain("Workouts unavailable")
  expect(container.textContent).not.toContain("Ready when you are.")
})

test("a logged workout and the strap's session on its day share one plate; the strap's other sessions stand alone", async () => {
  await api(workoutDetail, "/api/workouts", {
    method: "POST",
    body: JSON.stringify({
      date: "2026-09-30",
      name: "Mobility day",
      notes: null,
      exercises: [],
    }),
  })
  await api(workoutDetail, "/api/workouts", {
    method: "POST",
    body: JSON.stringify({
      date: "2026-09-12",
      name: "Quiet day",
      notes: null,
      exercises: [],
    }),
  })

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
  // The paired plate carries both halves and a way into the strap's full detail.
  const paired = container.querySelector('.session[data-kind="paired"]')

  expect(paired?.querySelector(".session-logged")).not.toBeNull()
  expect(paired?.querySelector(".session-strap")).not.toBeNull()
  expect(paired?.querySelector("a")?.getAttribute("href")).toContain(
    "/recovery?"
  )
})
