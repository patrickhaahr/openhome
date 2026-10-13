// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// The private server transport runs here too; browser CORS does not apply to Axum calls.

import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, expect, test, vi } from "vitest"
import { z } from "zod"
import { ExerciseProgress } from "@/components/exercise-progress"
import type { ProgressSearch } from "@/lib/exercise-progress"
import { readProgress } from "@/lib/exercise-progress.server"
import { exercise, trainingDayOn } from "@/lib/workout"
import { startAxumFixture } from "./support/axum-fixture"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let search: ProgressSearch

let renderWork: Promise<void>

const auth = {
  Authorization: "Bearer synthetic-dashboard-api-key",
  "Content-Type": "application/json",
}

beforeEach(async () => {
  fixture = await startAxumFixture()
  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
  search = {}
  renderWork = Promise.resolve()
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
  await fixture.stop()
})

/** A Training Day `days` before today, so the default three-month period covers it. */
function daysAgo(days: number): string {
  const today = Date.parse(`${trainingDayOn(new Date())}T00:00:00Z`)

  return new Date(today - days * 86_400_000).toISOString().slice(0, 10)
}

async function libraryId(name: string): Promise<number> {
  const response = await fetch(`${fixture.url}/api/exercises`, {
    headers: auth,
  })

  const match = exercise
    .array()
    .parse(await response.json())
    .find((item) => item.name === name)

  if (match === undefined) throw new Error(`Missing exercise: ${name}`)

  return match.id
}

type SyntheticSet = {
  readonly reps?: number
  readonly duration_seconds?: number
  readonly weight_kg?: number
}

async function log(
  date: string,
  entries: ReadonlyArray<readonly [string, ReadonlyArray<SyntheticSet>]>
): Promise<void> {
  const exercises = []

  for (const [order, [name, sets]] of entries.entries())
    exercises.push({
      exercise_id: await libraryId(name),
      order_index: order,
      sets: sets.map((set, index) => ({ set_number: index + 1, ...set })),
    })

  const response = await fetch(`${fixture.url}/api/workouts`, {
    method: "POST",
    headers: auth,
    body: JSON.stringify({ date, exercises }),
  })

  z.literal(201).parse(response.status)
}

async function render(): Promise<void> {
  const data = await readProgress(search)

  function reload(next: ProgressSearch): Promise<void> {
    search = next
    renderWork = render()

    return renderWork
  }

  await act(async () => {
    root.render(
      <ExerciseProgress
        data={data}
        choosePeriod={(period) => reload({ ...search, period })}
        compare={(ids) => {
          const { compare: _previous, ...rest } = search

          return reload(
            ids.length === 0 ? rest : { ...rest, compare: [...ids] }
          )
        }}
        refresh={() => reload(search)}
      />
    )
  })
}

function button(name: string): HTMLButtonElement {
  const match = Array.from(document.querySelectorAll("button")).find(
    (element) =>
      (element.getAttribute("aria-label") ?? element.textContent.trim()) ===
      name
  )

  if (match === undefined) throw new Error(`Missing button: ${name}`)

  return match
}

async function click(name: string): Promise<void> {
  await act(async () => {
    button(name).click()
    await renderWork
  })
  await act(async () => renderWork)
}

function legendEntries(): HTMLButtonElement[] {
  return Array.from(
    container.querySelectorAll<HTMLButtonElement>(
      '[aria-label="Exercise lines"] button'
    )
  )
}

function legendName(entry: HTMLButtonElement): string {
  return entry.querySelector(".progress-legend-name")?.textContent ?? ""
}

/** Each legend line's name and whether its line is shown. */
function legend(): ReadonlyMap<string, boolean> {
  return new Map(
    legendEntries().map((entry) => [
      legendName(entry),
      entry.getAttribute("aria-pressed") === "true",
    ])
  )
}

function lines(shown: Record<string, boolean>): ReadonlyMap<string, boolean> {
  return new Map(Object.entries(shown))
}

async function toggleLine(name: string): Promise<void> {
  const entry = legendEntries().find((item) => legendName(item) === name)

  if (entry === undefined) throw new Error(`Missing line: ${name}`)

  await act(async () => entry.click())
}

function drawnLines(): number {
  return container.querySelectorAll("svg .progress-line").length
}

function observed(): string[][] {
  return Array.from(container.querySelectorAll("tbody tr"), (row) =>
    Array.from(row.children, (cell) => cell.textContent)
  )
}

function notCharted(): string[] {
  return Array.from(
    container.querySelectorAll(".progress-missing li > div"),
    (item) => item.textContent
  )
}

test("indexes bodyweight reps and holds to their baselines, keeps declines, and toggles each line", async () => {
  await log(daysAgo(30), [
    ["Pull-up", [{ reps: 8 }, { reps: 6 }]],
    ["Planche Hold", [{ duration_seconds: 20 }]],
  ])
  await log(daysAgo(20), [["Pull-up", [{ reps: 10 }]]])
  await log(daysAgo(2), [
    ["Pull-up", [{ reps: 6 }]],
    ["Planche Hold", [{ duration_seconds: 75 }, { duration_seconds: 30 }]],
    ["Converging Chest Press", [{ reps: 10, weight_kg: 40 }]],
  ])

  await render()

  expect(legend()).toEqual(lines({ "Pull-up": true, "Planche Hold": true }))
  expect(drawnLines()).toBe(2)
  expect(container.textContent).toContain("Best set reps · baseline 8 reps on")
  expect(container.textContent).toContain("Latest 6 reps · 75")
  expect(container.textContent).toContain("Latest 1:15 min · 375")
  // The most recently logged charted line is inspected first.
  expect(observed()).toEqual([
    [`${shortDay(daysAgo(30))} · baseline`, "20 s", "100"],
    [shortDay(daysAgo(2)), "1:15 min", "375"],
  ])
  // The weighted lift is listed with its reason instead of an invented line.
  expect(notCharted()).toEqual([
    "Converging Chest PressHas sets with added weight; weighted progress is not charted yet",
  ])

  await toggleLine("Pull-up")
  expect(legend()).toEqual(lines({ "Pull-up": false, "Planche Hold": true }))
  expect(drawnLines()).toBe(1)
  await toggleLine("Pull-up")
  expect(drawnLines()).toBe(2)

  const select = container.querySelector("select")

  if (select === null) throw new Error("Missing observed-values select")

  await act(async () => {
    select.value = String(await libraryId("Pull-up"))
    select.dispatchEvent(new Event("change", { bubbles: true }))
  })
  expect(observed()).toEqual([
    [`${shortDay(daysAgo(30))} · baseline`, "8 reps", "100"],
    [shortDay(daysAgo(20)), "10 reps", "125"],
    [shortDay(daysAgo(2)), "6 reps", "75"],
  ])
})

test("compared never-logged and out-of-period exercises get no line until logged in the period", async () => {
  await log(daysAgo(3), [["Dip", [{ reps: 12 }]]])
  await log(daysAgo(200), [["Ring Dip", [{ reps: 5 }]]])
  await log(daysAgo(150), [["Ring Dip", [{ reps: 7 }]]])

  await render()
  expect(legend()).toEqual(lines({ Dip: true }))
  await toggleLine("Dip")

  await click("Add exercise")
  await click("Add Ring Dip")
  await click("Add exercise")
  await click("Add Front Lever Raise")

  // A line turned off stays off while the compared exercises change.
  expect(legend()).toEqual(lines({ Dip: false }))
  expect(notCharted()).toEqual([
    "Ring DipNot logged in this period",
    "Front Lever RaiseNever logged",
  ])

  await click("12 months")
  expect(legend()).toEqual(lines({ Dip: false, "Ring Dip": true }))
  expect(notCharted()).toEqual(["Front Lever RaiseNever logged"])

  await click("Stop comparing Front Lever Raise")
  expect(notCharted()).toEqual([])
  await click("Most recent")
  expect(search).toEqual({ period: "12m" })
  expect(legend()).toEqual(lines({ Dip: false, "Ring Dip": true }))
})

test("an exercise added beyond the first six lines is shown, and a charted line can be removed", async () => {
  await log(daysAgo(5), [
    ["Pull-up", [{ reps: 8 }]],
    ["Dip", [{ reps: 10 }]],
    ["Planche Push-up", [{ reps: 4 }]],
    ["Planche Press", [{ reps: 3 }]],
    ["Front Lever Pull-up", [{ reps: 5 }]],
    ["Front Lever Touch", [{ reps: 6 }]],
    ["Handstand Push-up", [{ reps: 7 }]],
  ])
  await log(daysAgo(200), [["Ring Dip", [{ reps: 5 }]]])
  await log(daysAgo(150), [["Ring Dip", [{ reps: 7 }]]])

  await render()
  expect(legendEntries()).toHaveLength(7)
  expect(drawnLines()).toBe(6)

  await click("Add exercise")
  await click("Add Ring Dip")
  expect(notCharted()).toEqual(["Ring DipNot logged in this period"])
  // Charted once the period covers it, as the eighth line, and shown because it was chosen.
  await click("12 months")
  expect(legend().get("Ring Dip")).toBe(true)
  expect(drawnLines()).toBe(7)

  const select = container.querySelector("select")

  if (select === null) throw new Error("Missing observed-values select")

  await act(async () => {
    select.value = String(await libraryId("Ring Dip"))
    select.dispatchEvent(new Event("change", { bubbles: true }))
  })
  await click("Stop comparing Ring Dip")
  expect(legend().has("Ring Dip")).toBe(false)
  expect(search.compare).toHaveLength(7)
})

test("an unreachable read is unavailable rather than an empty chart", async () => {
  vi.stubEnv("OPENHOME_API_URL", "http://127.0.0.1:9")
  await render()
  expect(container.textContent).toContain("Progress unavailable")
  expect(container.textContent).not.toContain("Nothing logged")
  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  await click("Refresh progress")
  expect(container.textContent).toContain("Nothing logged in this period")
})

function shortDay(day: string): string {
  return new Intl.DateTimeFormat("en-GB", {
    day: "numeric",
    month: "short",
    timeZone: "UTC",
  }).format(new Date(`${day}T00:00:00Z`))
}
