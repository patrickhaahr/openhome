import { act, useState } from "react"
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, expect, vi } from "vitest"
import { WAKE_GRACE_MS } from "@/components/idle"
import { useIdleBlock } from "@/components/idle-block"
import type { IdleSources } from "@/components/idle-screen"
import type { DeckPage, IdleSettings } from "@/lib/idle"
import { unavailableWeather } from "@/lib/weather"
import type { Album, AlbumRead, WallRead } from "@/lib/photos"

export const TIMEOUT = 10_000

/** The documented contract: a held touch that never reports its release wakes after 3 seconds. */
export const LOST_RELEASE_WAKE_MS = 3000

export const settings: IdleSettings = { timeoutMs: TIMEOUT, preview: false }

export const albumId = "5eed0000-0000-4000-8001-000000000000"

export const asset = (n: number): string =>
  `5eed0000-0000-4000-8000-${String(n).padStart(12, "0")}`

export const album: Album = {
  id: albumId,
  name: "Synthetic scenes",
  photo_count: 3,
  cover_id: null,
  first_month: "2025-06",
  last_month: "2025-06",
  missing: false,
}

export const readyAlbums: AlbumRead = { state: "ready", albums: [album] }

export function pageOf(
  count: number,
  page = 0,
  next: number | null = null
): WallRead {
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

export function fakeSources(
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

export let container: HTMLDivElement

export let root: Root

let reduced = false

/** Whether the next matchMedia asks for reduced motion answers yes. */
export function setReduced(value: boolean): void {
  reduced = value
}

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

export async function advance(ms: number): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms)
  })
}

export function screen(): HTMLElement | null {
  return document.querySelector(".idle")
}

export async function render(node: React.ReactNode): Promise<void> {
  await act(async () => root.render(node))
}

export function fire(target: EventTarget, type: string): boolean {
  const event = type.startsWith("pointer")
    ? new PointerEvent(type, { bubbles: true, cancelable: true })
    : type.startsWith("key")
      ? new KeyboardEvent(type, { bubbles: true, cancelable: true, key: "a" })
      : new MouseEvent(type, { bubbles: true, cancelable: true })

  return target.dispatchEvent(event)
}

/** A page with a draft, a counter and a key, to prove waking returns to exactly this. */
export function Page({
  blocked = false,
}: {
  blocked?: boolean
}): React.JSX.Element {
  const [count, setCount] = useState(0)
  useIdleBlock(blocked)

  return (
    <div>
      <input aria-label="Draft" defaultValue="" />
      <button type="button" onClick={() => setCount(count + 1)}>
        Count {count}
      </button>
    </div>
  )
}

export function countKey(): HTMLButtonElement {
  const key = container.querySelector("button")

  if (key === null) throw new Error("Expected the count key")

  return key
}

export async function wake(): Promise<void> {
  const idle = screen()

  if (idle === null) throw new Error("Expected the slideshow")

  await act(async () => {
    fire(idle, "pointerdown")
    fire(idle, "pointerup")
    fire(idle, "click")
  })
  await advance(WAKE_GRACE_MS)
  expect(screen()).toBeNull()
}

export function slides(): HTMLImageElement[] {
  return Array.from(document.querySelectorAll(".idle-slide img"))
}

/** Renders `node` inside a router, for components that link. */
export async function renderRouted(node: React.ReactNode): Promise<void> {
  const router = createRouter({
    routeTree: createRootRoute({ component: () => node }),
    history: createMemoryHistory(),
  })

  await act(async () => {
    await router.load()
    root.render(<RouterProvider router={router} />)
  })
}

export function Holder({ held }: { held: boolean }): null {
  useIdleBlock(held)

  return null
}
