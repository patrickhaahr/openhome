// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// Axum requests here use the server transport, which is not subject to browser CORS.

import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, expect, test, vi } from "vitest"
import { TrainingSummaries } from "@/components/training-summaries"
import { trainingSearch, type TrainingSearch } from "@/lib/training"
import { readRecoveryDays, readTraining } from "@/lib/training.server"
import { writeWorkout } from "@/lib/workout.server"
import { writeBodyProfile } from "@/lib/body-profile.server"
import { startAxumFixture } from "./support/axum-fixture"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let search: TrainingSearch

let pending: Promise<void>

beforeEach(async () => {
  fixture = await startAxumFixture()
  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
  search = { day: "2026-09-30" }
  pending = Promise.resolve()
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  await fixture.stop()
})

async function render(): Promise<void> {
  const data = await readTraining(search)
  await act(async () => {
    root.render(
      <TrainingSummaries
        key={data.day}
        data={data}
        selectDay={(day) => {
          search = { day }
          pending = render()

          return pending
        }}
        open={(noop) => {
          search = { ...search, noop }
          pending = render()

          return pending
        }}
        refresh={() => {
          pending = render()

          return pending
        }}
      />
    )
  })
}

function buttonNamed(text: string): HTMLButtonElement {
  const button = Array.from(container.querySelectorAll("button")).find(
    (element) =>
      element.textContent.startsWith(text) ||
      element.getAttribute("aria-label")?.startsWith(text) === true
  )

  if (button === undefined) throw new Error(`Missing button: ${text}`)

  return button
}

async function click(text: string): Promise<void> {
  const button = buttonNamed(text)
  await act(async () => {
    button.click()
    await pending
  })
}

/** Re-selecting the pressed day reads it again, as the refresh key once did. */
async function refresh(): Promise<void> {
  const current = container.querySelector<HTMLButtonElement>(
    '.today-day[aria-pressed="true"]'
  )

  if (current === null) throw new Error("Missing selected day")

  await act(async () => {
    current.click()
    await pending
  })
}

function ring(label: string): string | null | undefined {
  return Array.from(container.querySelectorAll(".score-ring"))
    .map((element) => element.getAttribute("aria-label"))
    .find((name) => name?.startsWith(`${label}:`) === true)
}

async function closeDetail(): Promise<void> {
  const close = buttonNamed("Close NOOP detail")
  close.focus()
  await click("Close NOOP detail")
  await act(async () => {
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
  })
}

test("draws the selected day as rings, a week of days, its night and body, and its own sessions", async () => {
  const stored = await writeWorkout({
    kind: "create",
    date: "2026-09-30",
    name: "Synthetic pull",
    notes: null,
    exercises: [
      {
        exercise_id: 11,
        order_index: 0,
        notes: null,
        sets: [
          {
            set_number: 1,
            reps: 5,
            weight_kg: null,
            duration_seconds: null,
            rpe: 8,
            notes: null,
          },
        ],
      },
    ],
  })

  expect(stored.state).toBe("saved")
  await render()

  expect(ring("Charge")).toBe("Charge: 78 % recovery")
  expect(ring("Rest")).toBe("Rest: 87 % rest")
  expect(ring("Effort")).toBe("Effort: strain 35 of 100")
  // A strip of days, not a week with arrows.
  expect(container.querySelectorAll(".today-day")).toHaveLength(28)
  expect(container.textContent).not.toContain("Previous week")
  expect(container.textContent).not.toContain("Next week")
  expect(
    container
      .querySelector('.today-day[aria-pressed="true"]')
      ?.getAttribute("aria-label")
  ).toContain("Charge 78 %")

  const vitals = Array.from(container.querySelectorAll(".vital")).map(
    (element) => element.textContent
  )

  expect(vitals).toContain("Resting HR50bpm")
  expect(vitals).toContain("HRV62ms")
  expect(vitals.some((text) => text.startsWith("Skin tempNot recorded"))).toBe(
    true
  )
  expect(vitals).toContain("StepsNot recorded")
  // The night in time order: lanes awake to deep, segments clamped to the edited 23:05 start.
  expect(
    Array.from(container.querySelectorAll(".hypno-lane")).map((lane) => [
      lane.getAttribute("data-stage"),
      lane.children.length,
    ])
  ).toEqual([
    ["awake", 0],
    ["rem", 2],
    ["light", 2],
    ["deep", 2],
  ])
  expect(
    Array.from(container.querySelectorAll(".hypno-labels p")).map(
      (label) => label.textContent
    )
  ).toEqual(["Awake0:00", "REM1:40", "Light4:15", "Deep1:30"])
  // Bedtime, the night's sleep, then wake.
  expect(container.querySelector(".hypno-times")?.textContent).toBe(
    "23:057:3006:30"
  )
  expect(container.querySelector(".hypno-axis")?.textContent).toBe(
    "00:0001:0002:0003:0004:0005:0006:00"
  )
  expect(container.querySelector(".today-stages")).toBeNull()
  expect(
    Array.from(container.querySelectorAll(".hypno tbody tr")).map(
      (row) => row.textContent
    )
  ).toEqual([
    "Light23:0501:102 hours 5 minutes",
    "Deep01:1001:5545 minutes",
    "REM01:5502:4550 minutes",
    "Light02:4504:552 hours 10 minutes",
    "Deep04:5505:4045 minutes",
    "REM05:4006:3050 minutes",
  ])
  expect(container.textContent).toContain("Freshness: confirmed")
  expect(container.textContent).toContain("daily metrics: covered")
  expect(container.textContent).toContain("User edited")
  expect(container.textContent).toContain("Felt recovered: No")
  expect(container.textContent).toContain("Synthetic journal note")
  expect(container.querySelectorAll(".night-history-bars span")).toHaveLength(
    30
  )
  expect(buttonNamed("Open Run / walk")).toBeDefined()
  // Logged Workouts live in Workout › Log, not beside the strap's recordings.
  expect(container.textContent).not.toContain("Synthetic pull")
  expect(container.textContent).not.toContain("Open logged")
  expect(container.textContent).not.toContain("Refresh training")

  await click("Tue 29 Sept")
  expect(search.day).toBe("2026-09-29")
  expect(container.textContent).toContain(
    "No NOOP workouts recorded on this day."
  )
})

test("opens the exact sport at a colliding start, shows strap detail and Profile-dependent zones/recovery", async () => {
  await render()
  await click("Open Run / walk")
  expect(document.activeElement?.getAttribute("aria-label")).toBe(
    "NOOP workout detail"
  )
  expect(container.textContent).toContain("Average pace5:00 /km")
  expect(container.textContent).toContain(
    "Heart rate basis: strap samples (my-whoop)"
  )
  expect(container.textContent).toContain(
    "Set Maximum heart rate in Body to enable this read."
  )
  await writeBodyProfile({ kind: "profile", hr_max_bpm: 190 })
  await refresh()
  expect(container.textContent).toContain("Profile maximum: 190 beats/min")
  expect(container.textContent).toContain("End heart rate: 165 bpm")
  expect(container.textContent).toContain("120 beats/min · Drop: 45")
  expect(container.textContent).toContain(
    "Too few strap samples; not interpolated."
  )
  expect(container.querySelectorAll("meter")).toHaveLength(5)

  const points = Array.from(
    container.querySelectorAll(".training-series-values p")
  ).map((point) => point.textContent)

  expect(points).toContain("+75 s: 150 beats/min")
  expect(points).not.toContain("+90 s: 150 beats/min")
  expect(points).toContain("+150 s: 150 beats/min")
  await click("Open Cycling")
  expect(container.querySelector(".noop-detail h2")?.textContent).toBe(
    "Cycling"
  )
  expect(search.noop?.sport).toBe("Cycling")
  await click("Open Mobility")
  expect(container.textContent).toContain(
    "stored workout row (my-whoop); unreconciled with the strap"
  )
  expect(container.textContent).toContain(
    "No strap heart rate samples are available"
  )
  await closeDetail()
  expect(document.activeElement).toBe(buttonNamed("Open Mobility"))
  expect(container.querySelector(".noop-detail")).toBeNull()
})

test("day selection distinguishes no sync from recorded absence and allows refresh after an authorization failure", async () => {
  search = { day: "2026-10-03" }
  await render()
  expect(container.textContent).toContain("Freshness: unknown")
  expect(ring("Rest")).toBe("Rest: Not synced or unavailable")
  expect(container.textContent).toContain("NOOP workouts have unknown coverage")
  await click("Tue 29 Sept")
  expect(container.textContent).toContain("Recovery day · 2026-09-29")
  expect(ring("Rest")).toBe("Rest: Not recorded")
  expect(container.textContent).toContain(
    "No sleep sessions recorded for this night."
  )
  vi.stubEnv("API_KEY", "wrong-private-credential")
  await refresh()
  expect(container.textContent).toContain("Recovery day unavailable")
  expect(container.textContent).toContain("Training context unavailable")
  expect(container.textContent).toContain("OpenHome rejected the API key")
  expect(container.textContent).not.toContain("wrong-private-credential")
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  await refresh()
  expect(container.textContent).toContain("Recovery day · 2026-09-29")
})

test("the day strip reaches into older days as it is scrolled and keeps every day a button", async () => {
  const loadDays = vi.fn(readRecoveryDays)
  search = { day: "2026-09-30" }
  const data = await readTraining(search)
  await act(async () => {
    root.render(
      <TrainingSummaries
        key={data.day}
        data={data}
        selectDay={async () => {}}
        open={async () => {}}
        refresh={async () => {}}
        loadDays={loadDays}
      />
    )
  })

  // Days after the selected one fill in toward today first.
  await act(async () => {
    await vi.waitFor(() => expect(loadDays).toHaveBeenCalledTimes(1))
    await new Promise<void>((resolve) => setTimeout(resolve, 300))
  })

  const strip = container.querySelector(".today-days")
  const before = container.querySelectorAll("button.today-day").length

  if (strip === null) throw new Error("Missing day strip")
  expect(strip.querySelectorAll("li > button")).toHaveLength(before)
  await act(async () => {
    strip.dispatchEvent(new Event("scroll"))
    await new Promise<void>((resolve) => setTimeout(resolve, 300))
  })
  expect(container.querySelectorAll("button.today-day").length).toBeGreaterThan(
    before
  )

  const firstDay = container
    .querySelector("button.today-day")
    ?.getAttribute("aria-label")

  expect(firstDay).toContain("Rest")
  expect(loadDays).toHaveBeenCalledTimes(2)
})

test("a missing or rejected detail and a failed context leave independent recovery data visible", async () => {
  search.noop = {
    source: "my-whoop",
    start: "2026-09-30T11:00:00+02:00",
    sport: "Run / walk",
  }
  await render()
  expect(container.textContent).toContain(
    "This NOOP workout is no longer available"
  )
  expect(ring("Rest")).toBe("Rest: 87 % rest")
  const realFetch = globalThis.fetch
  vi.spyOn(globalThis, "fetch").mockImplementation((input, options) =>
    (input instanceof Request ? input.url : input.toString()).includes(
      "/api/training/context/"
    )
      ? Promise.resolve(
          Response.json(
            { error: "private-diagnostic", status: 422 },
            { status: 422 }
          )
        )
      : realFetch(input, options)
  )
  await refresh()
  expect(container.textContent).toContain(
    "This read exceeds OpenHome's data limit"
  )
  expect(container.textContent).not.toContain("private-diagnostic")
  expect(ring("Rest")).toBe("Rest: 87 % rest")

  for (const source of [".", "..", "\u0000"]) {
    const data = await readTraining({
      day: "2026-09-30",
      noop: { source, start: "2026-09-30T12:00:00+02:00", sport: "Cycling" },
    })

    expect(data.detail).toEqual({ state: "unavailable", reason: "invalid" })
  }

  expect(
    trainingSearch.parse({ day: "2026-02-30", noop: { source: "my-whoop" } })
  ).toEqual({ day: undefined, noop: undefined })
})

test("closing a detail restores its session button, falling back to the selected day when it is gone", async () => {
  search.day = "2026-10-01"
  search.noop = {
    source: "my-whoop",
    start: "2026-09-30T12:00:00+02:00",
    sport: "Run / walk",
  }
  await render()
  // A deep link to another day's session has no button on this day.
  await closeDetail()
  expect(document.activeElement?.getAttribute("aria-pressed")).toBe("true")
  expect(document.activeElement?.className).toBe("today-day")

  await click("Wed 30 Sept")
  await click("Open Cycling")
  const realFetch = globalThis.fetch

  const failedContext = vi
    .spyOn(globalThis, "fetch")
    .mockImplementation((input, options) =>
      (input instanceof Request ? input.url : input.toString()).includes(
        "/api/training/context/"
      )
        ? Promise.resolve(new Response(null, { status: 500 }))
        : realFetch(input, options)
    )

  await refresh()
  expect(container.textContent).toContain("Training context unavailable")
  failedContext.mockRestore()
  await refresh()
  await closeDetail()
  expect(document.activeElement).toBe(buttonNamed("Open Cycling"))

  await click("Open Mobility")
  vi.stubEnv("API_KEY", "wrong-private-credential")
  await refresh()
  await closeDetail()
  expect(document.activeElement?.className).toBe("today-day")
})
