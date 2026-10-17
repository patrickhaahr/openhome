// @vitest-environment happy-dom
import { act, useState } from "react"
import { createRoot, type Root } from "react-dom/client"
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router"
import { afterEach, beforeEach, expect, test, vi } from "vitest"
import { IdleProvider, WAKE_GRACE_MS } from "@/components/idle"
import { useIdleBlock } from "@/components/idle-block"
import {
  DWELL_MS,
  FADE_MS,
  REST_MS,
  type IdleSources,
} from "@/components/idle-screen"
import { LightControls } from "@/components/light-controls"
import {
  dealPages,
  parseIdleSettings,
  slideSource,
  wantsIdleNow,
  type DeckPage,
  type IdleSettings,
} from "@/lib/idle"
import type { LightCommand, LightCommandResult } from "@/lib/lights"
import {
  unavailablePhotos,
  type Album,
  type AlbumRead,
  type WallRead,
} from "@/lib/photos"
import { unavailableWeather } from "@/lib/weather"

const TIMEOUT = 10_000

const settings: IdleSettings = { timeoutMs: TIMEOUT, preview: false }

const albumId = "5eed0000-0000-4000-8001-000000000000"

const asset = (n: number): string =>
  `5eed0000-0000-4000-8000-${String(n).padStart(12, "0")}`

const album: Album = {
  id: albumId,
  name: "Synthetic scenes",
  photo_count: 3,
  cover_id: null,
  first_month: "2025-06",
  last_month: "2025-06",
  missing: false,
}

const readyAlbums: AlbumRead = { state: "ready", albums: [album] }

function pageOf(count: number, page = 0, next: number | null = null): WallRead {
  return {
    state: "ready",
    wall: {
      items: Array.from({ length: count }, (_, at) => ({
        album: albumId,
        asset: asset(page * 100 + at + 1),
      })),
      page,
      next,
      total: count,
    },
  }
}

function fakeSources(
  albums: () => Promise<AlbumRead> = async () => readyAlbums,
  page: (page: DeckPage) => Promise<WallRead> = async () => pageOf(3)
): IdleSources & { albumReads: () => number } {
  let reads = 0

  return {
    albums: () => {
      reads += 1

      return albums()
    },
    page,
    weather: async () => unavailableWeather,
    onThisDay: async () => ({
      state: "ready",
      onThisDay: { date: "10-17", items: [], page: 0, next: null, total: 0 },
    }),
    albumReads: () => reads,
  }
}

let container: HTMLDivElement

let root: Root

let reduced = false

beforeEach(() => {
  vi.useFakeTimers()
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  reduced = false

  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("reduce") && reduced,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))

  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

async function advance(ms: number): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms)
  })
}

function screen(): HTMLElement | null {
  return document.querySelector(".idle")
}

async function render(node: React.ReactNode): Promise<void> {
  await act(async () => root.render(node))
}

function fire(target: EventTarget, type: string): boolean {
  const event = type.startsWith("pointer")
    ? new PointerEvent(type, { bubbles: true, cancelable: true })
    : type.startsWith("key")
      ? new KeyboardEvent(type, { bubbles: true, cancelable: true, key: "a" })
      : new MouseEvent(type, { bubbles: true, cancelable: true })

  return target.dispatchEvent(event)
}

/** A page with a draft, a counter and a key, to prove waking returns to exactly this. */
function Page({ blocked = false }: { blocked?: boolean }): React.JSX.Element {
  const [count, setCount] = useState(0)
  useIdleBlock(blocked)

  return (
    <div className="page">
      <input aria-label="Draft" defaultValue="" />
      <button type="button" onClick={() => setCount(count + 1)}>
        Count {count}
      </button>
    </div>
  )
}

function countKey(): HTMLButtonElement {
  const key = container.querySelector("button")

  if (key === null) throw new Error("Expected the count key")

  return key
}

test("reads the timeout from the server setting, with a 5-minute default and an off switch", () => {
  expect(parseIdleSettings(undefined, undefined)).toEqual({
    timeoutMs: 300_000,
    preview: false,
  })
  expect(parseIdleSettings("90", undefined).timeoutMs).toBe(90_000)
  expect(parseIdleSettings("0", undefined).timeoutMs).toBeNull()
  expect(parseIdleSettings("soon", undefined).timeoutMs).toBe(300_000)
  expect(parseIdleSettings("2", undefined).timeoutMs).toBe(300_000)
  expect(parseIdleSettings("-5", undefined).timeoutMs).toBe(300_000)
  expect(parseIdleSettings("60", "true").preview).toBe(true)
  expect(parseIdleSettings("60", "yes").preview).toBe(false)
})

test("?idle=now works only where the server allows previews", () => {
  expect(wantsIdleNow(settings, "?idle=now")).toBe(false)
  expect(wantsIdleNow({ ...settings, preview: true }, "?idle=now")).toBe(true)
  expect(wantsIdleNow({ ...settings, preview: true }, "?idle=later")).toBe(
    false
  )
})

test("deals pages in a random but stable order and reads one page at a time", async () => {
  const albums: Album[] = [
    { ...album, id: albumId, photo_count: 130 },
    { ...album, id: asset(9), photo_count: 61 },
    { ...album, id: asset(8), photo_count: 40, missing: true },
  ]

  const pages = dealPages(albums, 42)
  expect(pages).toHaveLength(5)
  expect(dealPages(albums, 42)).toEqual(pages)
  expect(pages.every((page) => page.album !== asset(8))).toBe(true)

  const reads: DeckPage[] = []

  const source = slideSource(pages, 42, async (page) => {
    reads.push(page)

    return pageOf(2, page.page)
  })

  const first = [await source.next(), await source.next()]
  expect(reads).toHaveLength(1)
  await source.next()
  expect(reads).toHaveLength(2)

  const again = slideSource(pages, 42, async (page) => pageOf(2, page.page))
  expect([await again.next(), await again.next()]).toEqual(first)

  // A later spell carries on from where the last one stopped.
  const resumed = slideSource(
    pages,
    42,
    async (page) => pageOf(2, page.page),
    source.position()
  )

  expect(await resumed.next()).not.toEqual(first[0])
})

test("enters after the inactivity timeout, and activity restarts the countdown", async () => {
  await render(
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT - 1000)
  expect(screen()).toBeNull()
  fire(window, "pointermove")
  await advance(TIMEOUT - 1000)
  expect(screen()).toBeNull()
  await advance(1000)
  expect(screen()).not.toBeNull()
})

test("a timeout of zero never enters", async () => {
  await render(
    <IdleProvider
      settings={{ timeoutMs: null, preview: false }}
      sources={fakeSources()}
    >
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT * 100)
  expect(screen()).toBeNull()
})

test("a declared block holds the slideshow off; releasing it starts a full countdown", async () => {
  const sources = fakeSources()

  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page blocked />
    </IdleProvider>
  )

  await advance(TIMEOUT * 5)
  expect(screen()).toBeNull()

  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT - 1)
  expect(screen()).toBeNull()
  await advance(1)
  expect(screen()).not.toBeNull()

  // A block that appears while idle (a conversation starting) wakes the room.
  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page blocked />
    </IdleProvider>
  )

  expect(screen()).toBeNull()
})

test("the first touch only wakes, and returns to the page exactly as it was", async () => {
  const sources = fakeSources()

  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page />
    </IdleProvider>
  )

  const draft = container.querySelector("input")

  if (draft === null) throw new Error("Expected the draft")

  draft.value = "Half-written note"
  draft.focus()
  await act(async () => countKey().click())
  expect(countKey().textContent).toBe("Count 1")

  await advance(TIMEOUT)
  const idle = screen()

  if (idle === null) throw new Error("Expected the slideshow")

  expect(document.activeElement).toBe(idle)

  // The whole tap lands on the slideshow; even a stray click retargeted to the key is eaten.
  await act(async () => {
    fire(idle, "pointerdown")
    fire(idle, "pointerup")
    fire(idle, "click")
    fire(countKey(), "click")
  })

  expect(screen()?.dataset.waking).toBe("true")
  await advance(WAKE_GRACE_MS)
  expect(screen()).toBeNull()
  expect(countKey().textContent).toBe("Count 1")
  expect(draft.value).toBe("Half-written note")
  expect(document.activeElement).toBe(draft)

  await act(async () => countKey().click())
  expect(countKey().textContent).toBe("Count 2")
})

test("a key wakes without reaching the page", async () => {
  const keys: string[] = []

  await render(
    <IdleProvider settings={settings} sources={fakeSources()}>
      <div onKeyDown={(event) => keys.push(event.key)}>
        <Page />
      </div>
    </IdleProvider>
  )

  await advance(TIMEOUT)
  await act(async () => {
    fire(countKey(), "keydown")
    fire(countKey(), "keyup")
  })

  await advance(WAKE_GRACE_MS)
  expect(screen()).toBeNull()
  expect(keys).toEqual([])
})

test("a tap over Home's light key wakes the screen without sending a light command", async () => {
  const commands: LightCommand[] = []

  async function sendCommand({
    data,
  }: {
    data: LightCommand
  }): Promise<LightCommandResult> {
    commands.push(data)

    return { state: "accepted" }
  }

  const sources = fakeSources()

  const rootRoute = createRootRoute({
    component: () => (
      <IdleProvider settings={settings} sources={sources}>
        <LightControls
          layout="quick"
          connectionState="connected"
          sendCommand={sendCommand}
        />
      </IdleProvider>
    ),
  })

  const router = createRouter({
    routeTree: rootRoute,
    history: createMemoryHistory(),
  })

  await act(async () => {
    await router.load()
    root.render(<RouterProvider router={router} />)
  })

  const lightKey = container.querySelector<HTMLButtonElement>(
    '[data-command="on"]'
  )

  if (lightKey === null) throw new Error("Expected the light key")

  await advance(TIMEOUT)
  const idle = screen()

  if (idle === null) throw new Error("Expected the slideshow")

  // The browser hit-tests the slideshow; a late compatibility click may hit the key beneath.
  await act(async () => {
    fire(idle, "pointerdown")
    fire(idle, "pointerup")
    lightKey.click()
  })

  await advance(WAKE_GRACE_MS)
  expect(screen()).toBeNull()
  expect(commands).toEqual([])

  await act(async () => lightKey.click())
  expect(commands).toEqual(["on"])
})

test("crossfades between two image slots, preloading only the next photo", async () => {
  const pageReads: DeckPage[] = []

  const sources = fakeSources(undefined, async (page) => {
    pageReads.push(page)

    return pageOf(3)
  })

  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT)
  const images = () => Array.from(document.querySelectorAll(".idle-slide img"))
  expect(images()).toHaveLength(1)
  expect(
    document.querySelector(".idle-stage")?.getAttribute("data-motion")
  ).toBe("drift")

  await act(async () => fire(images()[0] ?? window, "load"))
  expect(document.querySelector('[data-front="true"]')).not.toBeNull()

  await advance(FADE_MS)
  expect(images()).toHaveLength(2)
  const preloading = images()[1]
  expect(preloading?.closest(".idle-slide")?.getAttribute("data-front")).toBe(
    "false"
  )

  await act(async () => fire(preloading ?? window, "load"))
  await advance(DWELL_MS - FADE_MS - 1)
  expect(preloading?.closest(".idle-slide")?.getAttribute("data-front")).toBe(
    "false"
  )
  await advance(1)
  expect(preloading?.closest(".idle-slide")?.getAttribute("data-front")).toBe(
    "true"
  )

  // After the fade the old slot takes the next photo: never more than two images.
  await advance(FADE_MS)
  expect(images()).toHaveLength(2)
  expect(pageReads).toHaveLength(1)
})

test("reduced motion keeps the crossfade and drops the drift", async () => {
  reduced = true

  await render(
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT)
  expect(
    document.querySelector(".idle-stage")?.getAttribute("data-motion")
  ).toBe("still")
})

test("an unavailable library rests on the plain ground with the clock, then quietly tries again", async () => {
  const sources = fakeSources(async () => unavailablePhotos)

  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT)
  expect(
    document.querySelector(".idle-stage")?.getAttribute("data-resting")
  ).toBe("true")
  expect(document.querySelectorAll(".idle-slide img")).toHaveLength(0)
  expect(document.querySelector(".idle-time")?.textContent).toMatch(
    /^\d\d:\d\d$/
  )
  expect(document.querySelector('[role="alert"]')).toBeNull()
  expect(sources.albumReads()).toBe(1)

  await advance(REST_MS)
  expect(sources.albumReads()).toBe(2)
})

test("a run of broken photos rests instead of showing an error", async () => {
  const sources = fakeSources(undefined, async () => pageOf(6))

  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT)

  for (let miss = 0; miss < 4; miss += 1)
    await act(async () =>
      fire(document.querySelector(".idle-slide img") ?? window, "error")
    )

  expect(
    document.querySelector(".idle-stage")?.getAttribute("data-resting")
  ).toBe("true")
  expect(document.querySelectorAll(".idle-slide img")).toHaveLength(0)
})

test("favours on-this-day photos first, labels only the shown photo, then keeps the normal two-slot rotation", async () => {
  const sources: IdleSources = {
    ...fakeSources(),
    onThisDay: async () => ({
      state: "ready",
      onThisDay: {
        date: "10-17",
        items: [{ album: albumId, asset: asset(999), year: 2023, yearsAgo: 3 }],
        page: 0,
        next: null,
        total: 1,
      },
    }),
  }

  vi.setSystemTime(new Date("2026-10-17T12:00:00Z"))
  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page />
    </IdleProvider>
  )
  await advance(TIMEOUT)
  const first = document.querySelector<HTMLImageElement>(".idle-slide img")
  expect(first?.src).toContain(asset(999))
  expect(document.querySelector(".idle-photo-memory")).toBeNull()
  await act(async () => fire(first ?? window, "load"))
  expect(document.querySelector(".idle-photo-memory")?.textContent).toBe(
    "3 years ago today"
  )
  await advance(FADE_MS)
  const images = document.querySelectorAll<HTMLImageElement>(".idle-slide img")
  expect(images).toHaveLength(2)
  expect(images[1]?.src).not.toContain(asset(999))
  await act(async () => fire(images[1] ?? window, "load"))
  await advance(DWELL_MS - FADE_MS)
  expect(document.querySelector(".idle-photo-memory")).toBeNull()
  expect(document.querySelectorAll(".idle-slide img")).toHaveLength(2)
})

test("an unavailable on-this-day read quietly uses the normal shuffle", async () => {
  const sources: IdleSources = {
    ...fakeSources(),
    onThisDay: async () => unavailablePhotos,
  }

  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page />
    </IdleProvider>
  )
  await advance(TIMEOUT)
  expect(document.querySelectorAll(".idle-slide img")).toHaveLength(1)
  expect(document.querySelector(".idle-photo-memory")).toBeNull()
})

test("Copenhagen midnight replaces yesterday's photo queue and clears its caption before loading today's memory", async () => {
  vi.setSystemTime(new Date("2026-10-17T21:59:48Z"))
  let reads = 0

  const sources: IdleSources = {
    ...fakeSources(),
    onThisDay: async () => {
      reads += 1

      return {
        state: "ready",
        onThisDay: {
          date: reads === 1 ? "10-17" : "10-18",
          items: [
            {
              album: albumId,
              asset: asset(reads === 1 ? 999 : 998),
              year: 2023,
              yearsAgo: 3,
            },
          ],
          page: 0,
          next: null,
          total: 1,
        },
      }
    },
  }

  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page />
    </IdleProvider>
  )
  await advance(TIMEOUT)
  await act(async () =>
    fire(document.querySelector(".idle-slide img") ?? window, "load")
  )
  expect(document.querySelector(".idle-photo-memory")?.textContent).toBe(
    "3 years ago today"
  )
  await advance(2000)
  expect(
    document.querySelector<HTMLImageElement>(".idle-slide img")?.src
  ).toContain(asset(998))
  expect(document.querySelector(".idle-photo-memory")).toBeNull()
  expect(document.querySelectorAll(".idle-slide img")).toHaveLength(1)
})
