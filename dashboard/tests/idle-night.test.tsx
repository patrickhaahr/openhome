// @vitest-environment happy-dom
import { act } from "react"
import { beforeEach, expect, test } from "vitest"
import { IdleProvider, NIGHT_CHECK_MS, WAKE_GRACE_MS } from "@/components/idle"
import { DEFAULT_IDLE_SECONDS, isNight, type IdleSettings } from "@/lib/idle"
import type { AlbumRead, WallRead } from "@/lib/photos"
import {
  advance,
  container,
  countKey,
  DAYTIME,
  fakeSources,
  fire,
  fireKey,
  LOST_RELEASE_WAKE_MS,
  Page,
  pageOf,
  readyAlbums,
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

test("an inactivity timeout between two clock checks uses the time at which it fires", async () => {
  await room()
  await advance(1)
  fire(window, "pointermove")
  await advance(TIMEOUT - 1)
  clock = NIGHTTIME
  await advance(1)
  expect(black()).not.toBeNull()
  expect(screen()).toBeNull()
  expect(sources.albumReads()).toBe(0)

  await wake(black)
  await turnTo(NIGHTTIME)
  await advance(1)
  fire(window, "pointermove")
  await advance(TIMEOUT - 1)
  clock = DAYTIME
  expect(black()).toBeNull()
  await advance(1)
  expect(black()).toBeNull()
  expect(screen()).not.toBeNull()
})

test("with the slideshow off, a timeout just after daybreak leaves the page showing", async () => {
  clock = NIGHTTIME
  await room({ settings: slideshowOff })
  await advance(1)
  fire(window, "pointermove")
  await advance(DEFAULT_TIMEOUT - 1)
  clock = DAYTIME
  await advance(1)
  expect(black()).toBeNull()
  expect(screen()).toBeNull()

  await turnTo(NIGHTTIME)
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

test.each([
  ["a short hold", WAKE_GRACE_MS * 2],
  ["a hold past the wake limit", LOST_RELEASE_WAKE_MS * 2],
])(
  "%s on the black screen only wakes, and inactivity returns to black",
  async (_, hold) => {
    clock = NIGHTTIME
    await room()
    const draft = container.querySelector("input")
    draft?.focus()
    await advance(TIMEOUT)

    await act(async () => {
      fire(black() ?? window, "pointerdown")
      fire(black() ?? window, "touchstart")
    })
    await act(async () => fire(black() ?? window, "touchmove"))
    expect(black()?.dataset.waking).toBe("true")
    await advance(hold)

    const released: string[] = []

    for (const type of ["pointerup", "touchend", "mouseup", "click"])
      countKey().addEventListener(type, () => released.push(type))

    for (const type of ["pointerup", "touchend", "mouseup"])
      await act(async () => fire(countKey(), type))
    await advance(WAKE_GRACE_MS - 1)
    await act(async () => fire(countKey(), "click"))
    expect(released).toEqual([])
    expect(countKey().textContent).toBe("Count 0")

    await advance(WAKE_GRACE_MS)
    expect(black()).toBeNull()
    expect(document.activeElement).toBe(draft)

    await act(async () => countKey().click())
    expect(countKey().textContent).toBe("Count 1")

    await advance(TIMEOUT)
    expect(black()).not.toBeNull()
    expect(screen()).toBeNull()
  }
)

test("a cancelled touch on the black screen settles like a release", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)

  await act(async () => fire(black() ?? window, "pointerdown"))
  await advance(WAKE_GRACE_MS)
  await act(async () => {
    fire(black() ?? window, "pointercancel")
    fire(black() ?? window, "touchcancel")
  })
  await advance(WAKE_GRACE_MS - 1)
  expect(black()).not.toBeNull()
  await advance(1)
  expect(black()).toBeNull()
})

test("with two fingers on the black screen, only the last release settles", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  const second: PointerEventInit = { pointerId: 2, isPrimary: false }

  await act(async () => {
    fire(black() ?? window, "pointerdown")
    fire(black() ?? window, "pointerdown", second)
    fire(black() ?? window, "pointerup")
  })
  await advance(WAKE_GRACE_MS * 2)
  expect(black()).not.toBeNull()

  await act(async () => fire(black() ?? window, "pointerup", second))
  await advance(WAKE_GRACE_MS)
  expect(black()).toBeNull()
})

test("after a lost release, the next touch works normally", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  await act(async () => fire(black() ?? window, "pointerdown"))
  await advance(LOST_RELEASE_WAKE_MS)
  expect(black()).toBeNull()

  await act(async () => fire(countKey(), "pointerdown"))
  await act(async () => fire(countKey(), "pointerup"))
  await act(async () => countKey().click())
  expect(countKey().textContent).toBe("Count 1")
})

function watchKeys(): string[] {
  const reached: string[] = []

  for (const type of ["keydown", "keyup", "keypress"])
    countKey().addEventListener(type, (event) => {
      if (event instanceof KeyboardEvent)
        reached.push(`${event.type}:${event.code}`)
    })

  return reached
}

async function keyOn(
  target: EventTarget,
  type: "keydown" | "keyup" | "keypress",
  code: string,
  repeat = false
): Promise<void> {
  await act(async () => fireKey(target, type, code, repeat))
}

test("a Space held past the grace on the black screen wakes without activating the restored key", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  const reached = watchKeys()

  await keyOn(black() ?? window, "keydown", "Space")
  await advance(1000)
  expect(black()).not.toBeNull()

  await keyOn(countKey(), "keyup", "Space")
  await advance(WAKE_GRACE_MS - 1)
  expect(black()).not.toBeNull()
  await advance(1)
  expect(black()).toBeNull()
  expect(reached).toEqual([])

  await keyOn(countKey(), "keydown", "Space")
  await keyOn(countKey(), "keyup", "Space")
  expect(reached).toEqual(["keydown:Space", "keyup:Space"])
})

test("an Enter held with repeats on the black screen never reaches the restored key", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  const reached = watchKeys()

  await keyOn(black() ?? window, "keydown", "Enter")
  await advance(500)

  for (let repeats = 0; repeats < 5; repeats += 1) {
    await keyOn(countKey(), "keydown", "Enter", true)
    await keyOn(countKey(), "keypress", "Enter")
    await advance(33)
  }

  await keyOn(countKey(), "keyup", "Enter")
  await advance(WAKE_GRACE_MS)
  expect(black()).toBeNull()
  expect(reached).toEqual([])
  expect(countKey().textContent).toBe("Count 0")

  await act(async () => countKey().click())
  expect(countKey().textContent).toBe("Count 1")
})

test("a key that works normally after the black screen's key was released", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  const reached = watchKeys()

  await keyOn(black() ?? window, "keydown", "Space")
  await keyOn(black() ?? window, "keyup", "Space")
  await advance(WAKE_GRACE_MS)
  expect(black()).toBeNull()

  await keyOn(countKey(), "keydown", "KeyA")
  await keyOn(countKey(), "keyup", "KeyA")
  expect(reached).toEqual(["keydown:KeyA", "keyup:KeyA"])
})

test("after a lost keyup the page shows but that key stays swallowed until its keyup", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  const reached = watchKeys()

  await keyOn(black() ?? window, "keydown", "Space")
  await advance(LOST_RELEASE_WAKE_MS)
  expect(black()).toBeNull()

  await keyOn(countKey(), "keydown", "Space", true)
  await keyOn(countKey(), "keyup", "Space")
  expect(reached).toEqual([])

  await advance(WAKE_GRACE_MS)
  await keyOn(countKey(), "keydown", "Space")
  expect(reached).toEqual(["keydown:Space"])
})

test("after a lost keyup another key reaches the page while the waking key stays swallowed", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  const reached = watchKeys()

  await keyOn(black() ?? window, "keydown", "Space")
  await advance(LOST_RELEASE_WAKE_MS * 2)
  expect(black()).toBeNull()

  await keyOn(countKey(), "keydown", "KeyA")
  await keyOn(countKey(), "keyup", "KeyA")
  await keyOn(countKey(), "keydown", "Space", true)
  await keyOn(countKey(), "keypress", "Space")
  await keyOn(countKey(), "keyup", "Space")
  expect(reached).toEqual(["keydown:KeyA", "keyup:KeyA"])

  await advance(WAKE_GRACE_MS)
  await keyOn(countKey(), "keydown", "Space")
  expect(reached).toEqual(["keydown:KeyA", "keyup:KeyA", "keydown:Space"])
})

test("after a lost keyup a fresh press of the same key works normally", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  const reached = watchKeys()

  await keyOn(black() ?? window, "keydown", "Space")
  await advance(LOST_RELEASE_WAKE_MS)
  expect(black()).toBeNull()

  await keyOn(countKey(), "keydown", "Space", true)
  expect(reached).toEqual([])

  await keyOn(countKey(), "keydown", "Space")
  await keyOn(countKey(), "keypress", "Space")
  await keyOn(countKey(), "keyup", "Space")
  expect(reached).toEqual(["keydown:Space", "keypress:Space", "keyup:Space"])
})

test("an Idle Block that ends a waking key still swallows its repeats and keyup", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  await keyOn(black() ?? window, "keydown", "Space")
  await room({ blocked: true })
  expect(black()).toBeNull()
  const reached = watchKeys()

  await keyOn(countKey(), "keydown", "Space", true)
  await keyOn(countKey(), "keypress", "Space")
  await keyOn(countKey(), "keyup", "Space")
  expect(reached).toEqual([])

  await advance(WAKE_GRACE_MS)
  await keyOn(countKey(), "keydown", "Space")
  expect(reached).toEqual(["keydown:Space"])
})

test("an Idle Block that ends a waking touch still swallows its release", async () => {
  clock = NIGHTTIME
  await room()
  await advance(TIMEOUT)
  await act(async () => fire(black() ?? window, "pointerdown"))
  await room({ blocked: true })
  expect(black()).toBeNull()

  await act(async () => fire(countKey(), "pointerup"))
  await act(async () => countKey().click())
  expect(countKey().textContent).toBe("Count 0")

  await advance(WAKE_GRACE_MS)
  await act(async () => countKey().click())
  expect(countKey().textContent).toBe("Count 1")
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

test("nightfall cancels the slideshow's reads and starts no further one", async () => {
  let answerAlbums: (read: AlbumRead) => void = () => undefined
  let albumSignal: AbortSignal | undefined
  let pageReads = 0

  sources = fakeSources(
    (signal) =>
      new Promise<AlbumRead>((resolve) => {
        albumSignal = signal
        answerAlbums = resolve
      }),
    async () => {
      pageReads += 1

      return pageOf(3)
    }
  )
  await room()
  await advance(TIMEOUT)
  expect(albumSignal?.aborted).toBe(false)

  await turnTo(NIGHTTIME)
  expect(albumSignal?.aborted).toBe(true)
  await act(async () => answerAlbums(readyAlbums))
  await advance(1)
  expect(pageReads).toBe(0)
})

test("nightfall cancels a photo page that is being read", async () => {
  let pageSignal: AbortSignal | undefined

  sources = fakeSources(undefined, (_, signal) => {
    pageSignal = signal

    return new Promise<WallRead>(() => undefined)
  })
  await room()
  await advance(TIMEOUT)
  expect(pageSignal?.aborted).toBe(false)

  await turnTo(NIGHTTIME)
  expect(pageSignal?.aborted).toBe(true)
})
