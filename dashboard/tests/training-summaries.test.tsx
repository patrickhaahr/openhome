// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// Axum requests here use the server transport, which is not subject to browser CORS.

import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, expect, test, vi } from "vitest"
import { TrainingSummaries } from "@/components/training-summaries"
import { trainingSearch, type TrainingSearch } from "@/lib/training"
import { readTraining } from "@/lib/training.server"
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

async function click(text: string): Promise<void> {
  const button = Array.from(container.querySelectorAll("button")).find(
    (element) => element.textContent?.startsWith(text)
  )

  if (button === undefined) throw new Error(`Missing button: ${text}`)
  await act(async () => {
    button.click()
    await pending
  })
}

test("renders API-resolved recovery sources and date-aligned logged/NOOP summaries without pairing them", async () => {
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

  const observations = Array.from(container.querySelectorAll("dl > div")).map(
    (element) => element.textContent
  )

  expect(observations).toContain("Sleep duration450 minSource: my-whoop-noop")
  expect(observations).toContain(
    "Resting heart rate50 beats/minSource: my-whoop"
  )
  expect(observations).toContain(
    "HRV · nightly RMSSD62 msSource: my-whoop-noop"
  )
  expect(container.textContent).toContain("Freshness: confirmed")
  expect(container.textContent).toContain("daily metrics: covered")
  expect(container.textContent).toContain("User edited")
  expect(container.textContent).toContain(
    "Skin temperature deviationNot recordedUnit: degC"
  )
  expect(container.textContent).toContain("Felt recovered: No")
  expect(container.textContent).toContain("Synthetic journal note")
  expect(
    container.querySelectorAll(
      ".training-disclosure > summary span:first-child"
    )
  ).toHaveLength(7)
  expect(
    container.querySelector('section[aria-label="Logged Workouts 2026-09-30"]')
      ?.textContent
  ).toContain("Synthetic pull")
  expect(
    container.querySelector('section[aria-label="NOOP Workouts 2026-09-30"]')
      ?.textContent
  ).toContain("Open Run / walk")
  expect(
    container.querySelector('section[aria-label="NOOP Workouts 2026-09-29"]')
      ?.textContent
  ).toContain("No NOOP workouts recorded")
})

test("opens the exact sport at a colliding start, shows strap detail and Profile-dependent zones/recovery", async () => {
  await render()
  await click("Open Run / walk")
  expect(document.activeElement?.getAttribute("aria-label")).toBe(
    "NOOP workout detail"
  )
  expect(container.textContent).toContain("Average pace300 s/km")
  expect(container.textContent).toContain(
    "Heart rate basis: strap samples (my-whoop)"
  )
  expect(container.textContent).toContain(
    "Set Maximum heart rate in Profile & weight"
  )
  await writeBodyProfile({ kind: "profile", hr_max_bpm: 190 })
  await click("Refresh training")
  expect(container.textContent).toContain("Profile maximum: 190 beats/min")
  expect(container.textContent).toContain("End heart rate: 165 beats/min")
  expect(container.textContent).toContain(
    "1 min: 120 beats/min · Drop: 45 beats/min"
  )
  expect(container.textContent).toContain(
    "5 min: Too few strap samples; not interpolated."
  )
  expect(container.querySelectorAll("meter")).toHaveLength(5)

  const points = Array.from(
    container.querySelectorAll(".training-series-values p")
  ).map((point) => point.textContent)

  expect(points).toContain("+75 s: 150 beats/min")
  expect(points).not.toContain("+90 s: 150 beats/min")
  expect(points).toContain("+150 s: 150 beats/min")
  await click("Open Cycling")
  expect(container.querySelectorAll("h2")[1]?.textContent).toBe("Cycling")
  expect(search.noop?.sport).toBe("Cycling")
  await click("Open Mobility")
  expect(container.textContent).toContain(
    "stored workout row (my-whoop); unreconciled with the strap"
  )
  expect(container.textContent).toContain(
    "No strap heart rate samples are available"
  )
  await click("Close NOOP detail")
  await act(async () => {
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
  })
  expect(document.activeElement).toBe(
    Array.from(container.querySelectorAll("button")).find((button) =>
      button.textContent?.startsWith("Open Mobility")
    )
  )
  expect(container.textContent).toContain("Explore a recorded session")
})

test("date selection distinguishes no sync from recorded absence and allows refresh after an authorization failure", async () => {
  search = { day: "2026-10-03" }
  await render()
  expect(container.textContent).toContain("Freshness: unknown")
  expect(container.textContent).toContain(
    "Sleep durationNot synced or unavailableUnit: min"
  )
  expect(container.textContent).toContain("NOOP workouts have unknown coverage")
  const input = container.querySelector("input")

  if (input === null) throw new Error("Missing Training day")
  await act(async () => {
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value"
    )?.set?.call(input, "2026-09-29")
    input.dispatchEvent(new Event("input", { bubbles: true }))
  })
  await click("Show day")
  expect(container.textContent).toContain("Recovery day · 2026-09-29")
  expect(container.textContent).toContain("Sleep durationNot recordedUnit: min")
  expect(container.textContent).toContain(
    "No sleep sessions recorded for this night."
  )
  vi.stubEnv("API_KEY", "wrong-private-credential")
  await click("Refresh training")
  expect(container.textContent).toContain("Recovery day unavailable")
  expect(container.textContent).toContain("Training context unavailable")
  expect(container.textContent).toContain("OpenHome rejected the API key")
  expect(container.textContent).not.toContain("wrong-private-credential")
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  await click("Refresh training")
  expect(container.textContent).toContain("Recovery day · 2026-09-29")
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
  expect(container.textContent).toContain("Sleep duration450 min")
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
  await click("Refresh training")
  expect(container.textContent).toContain(
    "This read exceeds OpenHome's data limit"
  )
  expect(container.textContent).not.toContain("private-diagnostic")
  expect(container.textContent).toContain("Sleep duration450 min")

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

test("closing a deep-linked or refreshed detail restores a current button, with a toolbar fallback when context is unavailable", async () => {
  search.day = "2026-10-01"
  search.noop = {
    source: "my-whoop",
    start: "2026-09-30T12:00:00+02:00",
    sport: "Run / walk",
  }
  await render()

  async function closeFromKeyboard(): Promise<void> {
    const close = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "Close NOOP detail"
    )

    if (close === undefined) throw new Error("Missing close button")
    close.focus()
    await click("Close NOOP detail")
    await act(async () => {
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => resolve())
      )
    })
  }

  await closeFromKeyboard()
  expect(document.activeElement).toBe(
    Array.from(container.querySelectorAll("button")).find((button) =>
      button.textContent?.startsWith("Open Run / walk")
    )
  )
  expect(document.activeElement?.closest("details")?.open).toBe(true)

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

  await click("Refresh training")
  expect(container.textContent).toContain("Training context unavailable")
  failedContext.mockRestore()
  await click("Refresh training")
  await closeFromKeyboard()
  expect(document.activeElement).toBe(
    Array.from(container.querySelectorAll("button")).find((button) =>
      button.textContent?.startsWith("Open Cycling")
    )
  )

  await click("Open Mobility")
  vi.stubEnv("API_KEY", "wrong-private-credential")
  await click("Refresh training")
  await closeFromKeyboard()
  expect(document.activeElement?.textContent).toBe("Show day")
})
