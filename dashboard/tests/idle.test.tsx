// @vitest-environment happy-dom
import { act } from "react"
import { expect, test, vi } from "vitest"
import { IdleProvider, WAKE_GRACE_MS } from "@/components/idle"
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
} from "@/lib/idle"
import type { LightCommand, LightCommandResult } from "@/lib/lights"
import { unavailablePhotos, type Album, type WallRead } from "@/lib/photos"
import {
  advance,
  album,
  albumId,
  asset,
  container,
  countKey,
  fakeSources,
  fire,
  Holder,
  LOST_RELEASE_WAKE_MS,
  Page,
  pageOf,
  render,
  renderRouted,
  root,
  screen,
  setReduced,
  settings,
  slideOf,
  slides,
  slot,
  TIMEOUT,
  wake,
} from "./support/idle-dom"

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

test("a held touch keeps swallowing until its release, and a lost release still wakes at the limit", async () => {
  await render(
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT)
  const idle = screen()

  if (idle === null) throw new Error("Expected the slideshow")

  await act(async () => fire(idle, "pointerdown"))

  await advance(WAKE_GRACE_MS * 2)
  expect(screen()?.dataset.waking).toBe("true")

  await act(async () => {
    fire(idle, "pointerup")
    fire(countKey(), "click")
  })
  expect(countKey().textContent).toBe("Count 0")
  await advance(WAKE_GRACE_MS)
  expect(screen()).toBeNull()

  await advance(TIMEOUT)
  await act(async () => fire(screen() ?? window, "pointerdown"))
  await advance(LOST_RELEASE_WAKE_MS - 1)
  expect(screen()).not.toBeNull()
  await advance(1)
  expect(screen()).toBeNull()
})

test("blocks are independent reasons: the countdown restarts only when the last is released", async () => {
  const tree = (first: boolean, second: boolean): React.ReactNode => (
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page />
      <Holder held={first} />
      <Holder held={second} />
    </IdleProvider>
  )

  await render(tree(true, true))
  await advance(TIMEOUT * 3)
  expect(screen()).toBeNull()

  await render(tree(false, true))
  await advance(TIMEOUT * 3)
  expect(screen()).toBeNull()

  await render(tree(false, false))
  await advance(TIMEOUT - 1)
  expect(screen()).toBeNull()
  await advance(1)
  expect(screen()).not.toBeNull()
})

test("a block that appears while waking takes the room back and returns focus", async () => {
  const tree = (blocked: boolean): React.ReactNode => (
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page blocked={blocked} />
    </IdleProvider>
  )

  await render(tree(false))
  const draft = container.querySelector("input")
  draft?.focus()
  await advance(TIMEOUT)
  await act(async () => fire(screen() ?? window, "pointerdown"))
  expect(screen()?.dataset.waking).toBe("true")

  await render(tree(true))
  expect(screen()).toBeNull()
  expect(document.activeElement).toBe(draft)

  await advance(TIMEOUT * 3)
  expect(screen()).toBeNull()
})

test("unmounting while the slideshow shows stops swallowing input", async () => {
  await render(
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT)
  await act(async () => fire(screen() ?? window, "pointerdown"))
  await act(async () => root.render(null))

  expect(fire(window, "pointerdown")).toBe(true)
  expect(fire(window, "keydown")).toBe(true)
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

  await renderRouted(
    <IdleProvider settings={settings} sources={sources}>
      <LightControls
        layout="quick"
        connectionState="connected"
        sendCommand={sendCommand}
      />
    </IdleProvider>
  )

  const lightKey = container.querySelector<HTMLButtonElement>(
    '[data-command="on"]'
  )

  if (lightKey === null) throw new Error("Expected the light key")

  await advance(TIMEOUT)
  const idle = screen()

  if (idle === null) throw new Error("Expected the slideshow")

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
  const images = () => slides()
  expect(images()).toHaveLength(1)
  expect(slot("idle-stage")?.getAttribute("data-motion")).toBe("drift")

  await act(async () => fire(images()[0] ?? window, "load"))
  expect(document.querySelector('[data-front="true"]')).not.toBeNull()

  await advance(FADE_MS)
  expect(images()).toHaveLength(2)
  const preloading = images()[1]
  expect(slideOf(preloading)?.getAttribute("data-front")).toBe("false")

  await act(async () => fire(preloading ?? window, "load"))
  await advance(DWELL_MS - FADE_MS - 1)
  expect(slideOf(preloading)?.getAttribute("data-front")).toBe("false")
  await advance(1)
  expect(slideOf(preloading)?.getAttribute("data-front")).toBe("true")

  await advance(FADE_MS)
  expect(images()).toHaveLength(2)
  expect(pageReads).toHaveLength(1)
})

test("reduced motion keeps the crossfade and drops the drift", async () => {
  setReduced(true)

  await render(
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT)
  expect(slot("idle-stage")?.getAttribute("data-motion")).toBe("still")
})

test("an unavailable library rests on the plain ground with the clock, then quietly tries again", async () => {
  const sources = fakeSources(async () => unavailablePhotos)

  await render(
    <IdleProvider settings={settings} sources={sources}>
      <Page />
    </IdleProvider>
  )

  await advance(TIMEOUT)
  expect(slot("idle-stage")?.getAttribute("data-resting")).toBe("true")
  expect(slides()).toHaveLength(0)
  expect(slot("idle-time")?.textContent).toMatch(/^\d\d:\d\d$/)
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
    await act(async () => fire(slides()[0] ?? window, "error"))

  expect(slot("idle-stage")?.getAttribute("data-resting")).toBe("true")
  expect(slides()).toHaveLength(0)
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
  const first = slides()[0]
  expect(first?.src).toContain(asset(999))
  expect(slot("idle-photo-memory")).toBeNull()
  await act(async () => fire(first ?? window, "load"))
  expect(slot("idle-photo-memory")?.textContent).toBe("3 years ago today")
  await advance(FADE_MS)
  const images = slides()
  expect(images).toHaveLength(2)
  expect(images[1]?.src).not.toContain(asset(999))
  await act(async () => fire(images[1] ?? window, "load"))
  await advance(DWELL_MS - FADE_MS)
  expect(slot("idle-photo-memory")).toBeNull()
  expect(slides()).toHaveLength(2)
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
  expect(slides()).toHaveLength(1)
  expect(slot("idle-photo-memory")).toBeNull()
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
  await act(async () => fire(slides()[0] ?? window, "load"))
  expect(slot("idle-photo-memory")?.textContent).toBe("3 years ago today")
  await advance(2000)
  expect(slides()[0]?.src).toContain(asset(998))
  expect(slot("idle-photo-memory")).toBeNull()
  expect(slides()).toHaveLength(1)
})

test("a cancelled run's late page failure cannot blank the photo its replacement is loading", async () => {
  let failOldRead: (read: WallRead) => void = () => undefined

  const stale = fakeSources(
    undefined,
    () =>
      new Promise<WallRead>((resolve) => {
        failOldRead = resolve
      })
  )

  await render(
    <IdleProvider settings={settings} sources={stale}>
      <Page />
    </IdleProvider>
  )
  await advance(TIMEOUT)
  expect(slides()).toHaveLength(0)

  await render(
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page />
    </IdleProvider>
  )
  await advance(1)
  expect(slides()).toHaveLength(1)

  await act(async () => failOldRead(unavailablePhotos))
  await advance(1)
  expect(slot("idle-stage")?.getAttribute("data-resting")).toBe("false")
  expect(slides()).toHaveLength(1)

  await act(async () => fire(slides()[0] ?? window, "load"))
  expect(document.querySelector('[data-front="true"]')).not.toBeNull()
})

test("waking while the first photo is still loading leaves nothing behind for the next spell", async () => {
  vi.setSystemTime(new Date("2026-10-17T12:00:00Z"))

  await render(
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page />
    </IdleProvider>
  )
  await advance(TIMEOUT)
  const abandoned = slides()[0]
  expect(slides()).toHaveLength(1)

  await wake()
  await act(async () => fire(abandoned ?? window, "load"))
  await advance(1000)

  await advance(TIMEOUT)
  expect(slides()).toHaveLength(1)
  expect(slides()[0]?.src).toBe(abandoned?.src)
  await act(async () => fire(slides()[0] ?? window, "load"))
  expect(document.querySelector('[data-front="true"]')).not.toBeNull()
})

test("waking during the dwell stops the rotation and the next spell carries on with the unseen photo", async () => {
  vi.setSystemTime(new Date("2026-10-17T12:00:00Z"))

  await render(
    <IdleProvider settings={settings} sources={fakeSources()}>
      <Page />
    </IdleProvider>
  )
  await advance(TIMEOUT)
  const shown = slides()[0]?.src
  await act(async () => fire(slides()[0] ?? window, "load"))
  await advance(FADE_MS)
  const preloaded = slides()[1]
  await act(async () => fire(preloaded ?? window, "load"))
  await advance(DWELL_MS - FADE_MS - 1000)

  await wake()
  await advance(2000)
  expect(screen()).toBeNull()

  await advance(TIMEOUT)
  expect(slides()).toHaveLength(1)
  expect(slides()[0]?.src).toBe(preloaded?.src)
  expect(slides()[0]?.src).not.toBe(shown)
})
