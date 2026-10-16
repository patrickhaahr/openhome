// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// The private server transport runs here too; browser CORS does not apply to Axum calls.

import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, expect, test, vi } from "vitest"
import { z } from "zod"
import { BodyProfile } from "@/components/body-profile"
import {
  bodyWeight,
  profile,
  type BodyMutation,
  type BodyRange,
} from "@/lib/body-profile"
import { readBodyProfile, writeBodyProfile } from "@/lib/body-profile.server"
import { shiftDay } from "@/lib/training"
import { trainingDayOn } from "@/lib/workout"
import { startAxumFixture } from "./support/axum-fixture"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let range: BodyRange

let mutationWork: Promise<BodyMutation | undefined>

let refreshWork: Promise<void>

let afterWrite: (() => void) | undefined

beforeEach(async () => {
  fixture = await startAxumFixture()
  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
  range = {}
  mutationWork = Promise.resolve(undefined)
  refreshWork = Promise.resolve()
  afterWrite = undefined
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
  await fixture.stop()
})

async function api<T>(
  schema: z.ZodType<T>,
  path: string,
  json?: { height_cm?: number; sex?: string; hr_max_bpm?: number }
): Promise<T> {
  const response = await fetch(`${fixture.url}${path}`, {
    method: json === undefined ? "GET" : "PATCH",
    headers: {
      Authorization: "Bearer synthetic-dashboard-api-key",
      "Content-Type": "application/json",
    },
    body: json === undefined ? null : JSON.stringify(json),
  })

  if (!response.ok) throw new Error(`${path} answered ${response.status}`)

  return schema.parse(await response.json())
}

async function render(): Promise<void> {
  const data = await readBodyProfile(range)

  await act(async () => {
    root.render(
      <BodyProfile
        data={data}
        range={range}
        change={(command) => {
          const work = writeBodyProfile(command).then((result) => {
            afterWrite?.()

            return result
          })

          mutationWork = work

          return work
        }}
        refresh={() => {
          refreshWork = render()

          return refreshWork
        }}
        filter={(next) => {
          range = next
          refreshWork = render()

          return refreshWork
        }}
      />
    )
  })
}

function input(label: string): HTMLInputElement {
  const fieldLabel = Array.from(container.querySelectorAll("label")).find(
    (element) => element.textContent === label
  )

  const element = document.getElementById(fieldLabel?.htmlFor ?? "")

  if (!(element instanceof HTMLInputElement))
    throw new Error(`Missing input: ${label}`)

  return element
}

async function type(label: string, value: string): Promise<void> {
  await act(async () => {
    const field = input(label)
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value"
    )?.set?.call(field, value)
    field.dispatchEvent(new Event("input", { bubbles: true }))
  })
}

async function click(name: string): Promise<void> {
  const button = Array.from(container.querySelectorAll("button")).find(
    (element) => element.textContent === name
  )

  if (button === undefined) throw new Error(`Missing button: ${name}`)

  await act(async () => {
    button.click()
    await mutationWork
    await refreshWork
  })
}

test("configures the shared Profile, preserves concurrent untouched fields, and explicitly clears each value", async () => {
  await render()
  expect(container.textContent?.match(/Not configured/g)).toHaveLength(3)
  await click("Edit profile")
  await type("Height (cm)", "181,12345")
  await type("Sex (optional)", "female")
  await type("Maximum heart rate (bpm)", "190")
  await click("Save profile")

  expect(await api(profile, "/api/profile")).toEqual({
    height_cm: 181.12345,
    sex: "female",
    hr_max_bpm: 190,
  })
  expect(container.textContent).toContain("Maximum heart rate190 max")
  expect(container.textContent).toContain("Profile saved to OpenHome.")

  await click("Edit profile")
  // Another client changes a field this draft leaves untouched.
  await api(profile, "/api/profile", { hr_max_bpm: 195 })
  await type("Sex (optional)", "")
  await click("Save profile")
  expect(await api(profile, "/api/profile")).toEqual({
    height_cm: 181.12345,
    sex: null,
    hr_max_bpm: 195,
  })

  await click("Edit profile")
  await type("Height (cm)", "")
  await type("Maximum heart rate (bpm)", "")
  await click("Save profile")
  expect(await api(profile, "/api/profile")).toEqual({
    height_cm: null,
    sex: null,
    hr_max_bpm: null,
  })
  await render()
  expect(container.textContent?.match(/Not configured/g)).toHaveLength(3)
})

test("logs decimal-comma Body Weight, rejects duplicates without replacing stored values, and charts ranges ending today", async () => {
  await render()
  expect(container.textContent).toContain(
    "No body weight recorded in this date range."
  )

  const today = trainingDayOn(new Date())

  const [old, recent, latest] = [
    shiftDay(today, -200),
    shiftDay(today, -60),
    shiftDay(today, -5),
  ]

  for (const [date, weight] of [
    [recent, "75,125"],
    [old, "74,5"],
    [latest, "74,2"],
  ]) {
    if (date === undefined || weight === undefined)
      throw new Error("Missing synthetic value")
    await type("Weight date", date)
    await type("Body weight (kg)", weight)
    await click("Save body weight")
  }

  expect(
    Array.from(container.querySelectorAll("tbody tr")).map(
      (row) => row.textContent
    )
  ).toEqual([`${old}74.5 kg`, `${recent}75.125 kg`, `${latest}74.2 kg`])
  expect(container.querySelector(".body-figure")?.textContent).toBe(
    `Latest body weight, ${latest}: 74.2kg▼ Change since ${old}: 0.3`
  )
  expect(container.querySelectorAll(".weight-chart-dot")).toHaveLength(3)
  await type("Body weight (kg)", "73")
  await click("Save body weight")
  expect(container.querySelector('[role="alert"]')?.textContent).toContain(
    "Body weight is already recorded for this date."
  )
  expect(input("Body weight (kg)").value).toBe("73")
  expect(
    (await api(bodyWeight.array(), "/api/body_weight")).map(
      ({ date, weight_kg }) => ({ date, weight_kg })
    )
  ).toEqual([
    { date: old, weight_kg: 74.5 },
    { date: recent, weight_kg: 75.125 },
    { date: latest, weight_kg: 74.2 },
  ])

  await click("1M")
  expect(range).toEqual({ from: shiftDay(today, -30), to: undefined })
  expect(container.querySelector("tbody")?.textContent).toBe(`${latest}74.2 kg`)
  expect(
    container.querySelector('.body-range[aria-pressed="true"]')?.textContent
  ).toBe("1M")

  await click("3M")
  expect(container.querySelectorAll("tbody tr")).toHaveLength(2)

  await click("All")
  expect(container.querySelectorAll("tbody tr")).toHaveLength(3)
})

test("invalid fields keep drafts and stored values; API validation also remains authoritative", async () => {
  await api(profile, "/api/profile", {
    height_cm: 180.5,
    sex: "male",
    hr_max_bpm: 185,
  })
  await render()
  await click("Edit profile")
  await type("Height (cm)", "0")
  await click("Save profile")
  expect(document.activeElement).toBe(input("Height (cm)"))
  expect(container.textContent).toContain("Enter height in cm above zero")
  await type("Height (cm)", "181")
  await type("Maximum heart rate (bpm)", "241")
  await click("Save profile")
  expect(input("Height (cm)").value).toBe("181")
  expect(document.activeElement).toBe(input("Maximum heart rate (bpm)"))
  expect(await api(profile, "/api/profile")).toEqual({
    height_cm: 180.5,
    sex: "male",
    hr_max_bpm: 185,
  })

  await type("Body weight (kg)", "0")
  await click("Save body weight")
  expect(container.textContent).toContain("Enter body weight in kg above zero.")
  expect(await api(bodyWeight.array(), "/api/body_weight")).toEqual([])
  expect(await writeBodyProfile({ kind: "profile", hr_max_bpm: 99 })).toEqual({
    state: "failed",
    reason: "invalid",
  })
  expect(
    await writeBodyProfile({
      kind: "weight",
      date: "2026-02-30",
      weight_kg: 75,
    })
  ).toEqual({ state: "failed", reason: "invalid" })
  expect(await api(profile, "/api/profile")).toEqual({
    height_cm: 180.5,
    sex: "male",
    hr_max_bpm: 185,
  })
})

test("unavailable and unauthorized reads never masquerade as unconfigured data and recover on refresh", async () => {
  vi.stubEnv("API_KEY", "invalid-synthetic-key")
  await render()
  expect(container.textContent).toContain("OpenHome rejected the API key.")
  expect(container.textContent).not.toContain("Not configured")
  expect(container.textContent).not.toContain("No body weight recorded")

  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  await click("Refresh profile & weight")
  expect(container.textContent).toContain(
    "No body weight recorded in this date range."
  )
})

test("a failed read after saving weight preserves the active Profile draft and offers recovery with the write receipt", async () => {
  await render()
  await click("Edit profile")
  await type("Height (cm)", "182,75")
  await type("Body weight (kg)", "74,5")
  afterWrite = () => vi.stubEnv("API_KEY", "invalid-synthetic-key")
  await click("Save body weight")

  expect(container.textContent).toContain("Profile & weight unavailable")

  const refresh = Array.from(container.querySelectorAll("button")).find(
    (button) => button.textContent === "Refresh profile & weight"
  )

  expect(refresh?.disabled).toBe(false)
  expect(container.textContent).toContain("Body weight saved to OpenHome.")

  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  afterWrite = undefined
  await click("Refresh profile & weight")
  expect(input("Height (cm)").value).toBe("182,75")
  await click("Save profile")
  expect(await api(profile, "/api/profile")).toMatchObject({
    height_cm: 182.75,
  })
  expect(
    (await api(bodyWeight.array(), "/api/body_weight")).map(
      (entry) => entry.weight_kg
    )
  ).toEqual([74.5])
})

test("a confirmed Profile save remains visible if its follow-up read is unavailable", async () => {
  await render()
  await click("Edit profile")
  await type("Maximum heart rate (bpm)", "180")
  afterWrite = () => vi.stubEnv("API_KEY", "invalid-synthetic-key")
  await click("Save profile")
  expect(container.textContent).toContain("Profile & weight unavailable")
  expect(container.textContent).toContain("Profile saved to OpenHome.")
  expect(await api(profile, "/api/profile")).toMatchObject({ hr_max_bpm: 180 })
})
