// @vitest-environment happy-dom
import { act } from "react"
import { beforeEach, expect, test } from "vitest"
import { IdleProvider, NIGHT_CHECK_MS, WAKE_GRACE_MS } from "@/components/idle"
import { DEFAULT_IDLE_SECONDS, isNight, type IdleSettings } from "@/lib/idle"
import {
  advance,
  container,
  countKey,
  DAYTIME,
  fakeSources,
  fire,
  Page,
  render,
  screen,
  settings,
  slot,
  TIMEOUT,
  wake,
} from "./support/idle-dom"

const NIGHTTIME = new Date("2026-10-17T21:00:00Z")

const slideshowOff: IdleSettings = { timeoutMs: null, preview: false }

const DEFAULT_TIMEOUT = DEFAULT_IDLE_SECONDS * 1000

let clock = DAYTIME

let sources = fakeSources()

beforeEach(() => {
  clock = DAYTIME
  sources = fakeSources()
})

function now(): Date {
  return clock
}

function room(
  options: { blocked?: boolean; settings?: IdleSettings } = {}
): Promise<void> {
  return render(
    <IdleProvider
      settings={options.settings ?? settings}
      sources={sources}
      overlay={<p>Overlay</p>}
      now={now}
    >
      <Page blocked={options.blocked ?? false} />
    </IdleProvider>
  )
}

async function turnTo(instant: Date): Promise<void> {
  clock = instant
  await advance(NIGHT_CHECK_MS)
}

function black(): HTMLElement | null {
  return slot("idle-night")
}

test.each([
  ["a second before 23:00 in winter", "2026-01-15T21:59:59Z", false],
  ["23:00 in winter", "2026-01-15T22:00:00Z", true],
  ["midnight in winter", "2026-01-15T23:00:00Z", true],
  ["a second before 08:30 in winter", "2026-01-16T07:29:59Z", true],
  ["08:30 in winter", "2026-01-16T07:30:00Z", false],
  ["a second before 23:00 in summer", "2026-07-15T20:59:59Z", false],
  ["23:00 in summer", "2026-07-15T21:00:00Z", true],
  ["midnight in summer", "2026-07-15T22:00:00Z", true],
  ["a second before 08:30 in summer", "2026-07-16T06:29:59Z", true],
  ["08:30 in summer", "2026-07-16T06:30:00Z", false],
  ["23:00 before the clocks go forward", "2026-03-28T22:00:00Z", true],
  ["08:29 after the clocks go forward", "2026-03-29T06:29:59Z", true],
  ["08:30 after the clocks go forward", "2026-03-29T06:30:00Z", false],
  ["23:00 before the clocks go back", "2026-10-24T21:00:00Z", true],
  ["08:29 after the clocks go back", "2026-10-25T07:29:59Z", true],
  ["08:30 after the clocks go back", "2026-10-25T07:30:00Z", false],
])("reads %s as Copenhagen time", (_, instant, night) => {
  expect(isNight(new Date(instant))).toBe(night)
})

test("nightfall turns a running slideshow black at once, and daybreak brings it back", async () => {
  await room()
  await advance(TIMEOUT)
  expect(screen()).not.toBeNull()

  await turnTo(NIGHTTIME)
  expect(screen()).toBeNull()
  expect(black()).not.toBeNull()
  expect(document.activeElement).toBe(black())

  await turnTo(DAYTIME)
  expect(black()).toBeNull()
  expect(screen()).not.toBeNull()
  expect(document.activeElement).toBe(screen())
})

test("a Dashboard in use at nightfall goes black only after the inactivity timeout", async () => {
  await room()
  await advance(TIMEOUT - NIGHT_CHECK_MS)
  fire(window, "pointermove")
  await turnTo(NIGHTTIME)
  expect(black()).toBeNull()

  await advance(TIMEOUT - NIGHT_CHECK_MS - 1)
  expect(black()).toBeNull()
  await advance(1)
  expect(black()).not.toBeNull()
})

test("a night screen is empty and reads nothing", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT * 10)

  expect(black()?.childNodes).toHaveLength(0)
  expect(screen()).toBeNull()
  expect(document.body.textContent).not.toContain("Overlay")
  expect(sources.albumReads()).toBe(0)
})

test("a touch on the black screen only wakes, and inactivity returns to black", async () => {
  clock = NIGHTTIME
  await room()
  const draft = container.querySelector("input")
  draft?.focus()
  await advance(TIMEOUT)

  await act(async () => {
    fire(black() ?? window, "pointerdown")
    fire(black() ?? window, "pointerup")
    fire(countKey(), "click")
  })
  expect(black()?.dataset.waking).toBe("true")
  await advance(WAKE_GRACE_MS)
  expect(black()).toBeNull()
  expect(countKey().textContent).toBe("Count 0")
  expect(document.activeElement).toBe(draft)

  await act(async () => countKey().click())
  expect(countKey().textContent).toBe("Count 1")

  await advance(TIMEOUT)
  expect(black()).not.toBeNull()
  expect(screen()).toBeNull()
})

test("daybreak leaves a Dashboard in use alone, then inactivity starts the slideshow", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  await wake(black)

  await turnTo(DAYTIME)
  expect(black()).toBeNull()
  expect(screen()).toBeNull()

  await advance(TIMEOUT - NIGHT_CHECK_MS)
  expect(screen()).not.toBeNull()
})

test("an Idle Block holds the night screen off and takes a black screen back", async () => {
  clock = NIGHTTIME
  await room({ blocked: true })
  await advance(TIMEOUT * 5)
  expect(black()).toBeNull()

  await room()
  await advance(TIMEOUT)
  expect(black()).not.toBeNull()

  await room({ blocked: true })
  expect(black()).toBeNull()
})

test("with the slideshow off, an untouched Dashboard goes black at nightfall and returns at daybreak", async () => {
  await room({ settings: slideshowOff })
  await advance(DEFAULT_TIMEOUT)
  expect(screen()).toBeNull()

  await turnTo(NIGHTTIME)
  expect(black()).not.toBeNull()

  await turnTo(DAYTIME)
  expect(black()).toBeNull()
  expect(screen()).toBeNull()
})

test("with the slideshow off, a Dashboard in use at nightfall goes black after the default timeout", async () => {
  await room({ settings: slideshowOff })
  await advance(DEFAULT_TIMEOUT)
  fire(window, "pointermove")

  await turnTo(NIGHTTIME)
  expect(black()).toBeNull()

  await advance(DEFAULT_TIMEOUT - NIGHT_CHECK_MS - 1)
  expect(black()).toBeNull()
  await advance(1)
  expect(black()).not.toBeNull()
})
