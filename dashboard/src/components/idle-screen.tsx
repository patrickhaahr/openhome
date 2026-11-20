import { useEffect, useRef, useState, useSyncExternalStore } from "react"
import { IdleOverlay } from "@/components/idle-overlay"
import {
  copenhagenDay,
  dealPages,
  seedOf,
  slideSource,
  type DeckPage,
  type SlideMemory,
  type SlideSource,
} from "@/lib/idle"
import {
  photoUrl,
  unavailablePhotos,
  type AlbumRead,
  type WallItem,
  type WallRead,
  type OnThisDayResult,
} from "@/lib/photos"
import {
  getOnThisDay,
  getSlideshowAlbums,
  getWallPage,
} from "@/lib/photos.functions"
import { useCopenhagenDay } from "@/lib/use-copenhagen-day"
import { cn } from "@/lib/utils"
import type { WeatherRead } from "@/lib/weather"
import { getWeather } from "@/lib/weather.functions"

export const DWELL_MS = 12_000

export const FADE_MS = 2400

export const REST_MS = 60_000

const MISSES = 4

export type IdleSources = {
  readonly albums: () => Promise<AlbumRead>
  readonly page: (page: DeckPage) => Promise<WallRead>
  readonly weather: () => Promise<WeatherRead>
  readonly onThisDay: () => Promise<OnThisDayResult>
}

export const serverSources: IdleSources = {
  albums: () => getSlideshowAlbums(),
  page: (page) => getWallPage({ data: page }),
  weather: () => getWeather(),
  onThisDay: () => getOnThisDay({ data: {} }),
}

const reducedMotion = "(prefers-reduced-motion: reduce)"

function subscribeToMotion(onChange: () => void): () => void {
  const query = matchMedia(reducedMotion)
  query.addEventListener("change", onChange)

  return () => query.removeEventListener("change", onChange)
}

export function useReducedMotion(): boolean {
  return useSyncExternalStore(
    subscribeToMotion,
    () => matchMedia(reducedMotion).matches,
    () => true
  )
}

export function IdleScreen({
  ref,
  waking,
  memory,
  overlay,
  sources,
}: {
  ref: React.Ref<HTMLDivElement>
  waking: boolean
  memory: SlideMemory
  overlay: React.ReactNode
  sources: IdleSources
}): React.JSX.Element {
  const day = useCopenhagenDay()

  return (
    <div
      ref={ref}
      data-slot="idle"
      className="dark fixed inset-0 z-100 cursor-none touch-none overflow-hidden bg-ground text-ink outline-none select-none touch-callout-none data-[waking=true]:opacity-0 data-[waking=true]:transition-opacity data-[waking=true]:duration-mid data-[waking=true]:ease-app"
      data-waking={waking}
      role="dialog"
      aria-modal="true"
      aria-label="Photo slideshow"
      tabIndex={-1}
    >
      <Slideshow
        key={day}
        day={day}
        memory={memory}
        sources={sources}
        overlay={overlay}
      />
      <p className="sr-only">Touch the screen or press any key to return.</p>
    </div>
  )
}

type Layer = {
  readonly item: WallItem
  readonly key: number
  readonly phase: "loading" | "front" | "behind"
}

type Layers = readonly [Layer | null, Layer | null]

type Stage = { readonly layers: Layers; readonly resting: boolean }

const blankStage: Stage = { layers: [null, null], resting: false }

type Loaded = "loaded" | "failed" | "cancelled"

function raise(layers: Layers, slot: 0 | 1): Layers {
  const lift = (layer: Layer | null, own: boolean): Layer | null =>
    layer === null
      ? null
      : own
        ? { ...layer, phase: "front" }
        : layer.phase === "front"
          ? { ...layer, phase: "behind" }
          : layer

  return [lift(layers[0], slot === 0), lift(layers[1], slot === 1)]
}

function wait(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal.aborted) {
      resolve()

      return
    }

    function finish(): void {
      clearTimeout(timer)
      signal.removeEventListener("abort", finish)
      resolve()
    }

    const timer = setTimeout(finish, ms)
    signal.addEventListener("abort", finish)
  })
}

function Slideshow({
  day,
  memory,
  sources,
  overlay,
}: {
  overlay: React.ReactNode
  day: string
  memory: SlideMemory
  sources: IdleSources
}): React.JSX.Element {
  const still = useReducedMotion()
  const [stage, setStage] = useState<Stage>(blankStage)
  const { layers, resting } = stage
  const settlers = useRef(new Map<number, (loaded: boolean) => void>())
  const keys = useRef(0)

  useEffect(() => {
    const stop = new AbortController()
    const { signal } = stop

    function show(change: (previous: Stage) => Stage): void {
      if (!signal.aborted) setStage(change)
    }

    function place(slot: 0 | 1, layer: Layer): void {
      show(({ layers: previous, resting: ground }) => ({
        layers: slot === 0 ? [layer, previous[1]] : [previous[0], layer],
        resting: ground,
      }))
    }

    function load(slot: 0 | 1, item: WallItem): Promise<Loaded> {
      if (signal.aborted) return Promise.resolve("cancelled")
      keys.current += 1
      const key = keys.current

      return new Promise((resolve) => {
        function end(outcome: Loaded): void {
          settlers.current.delete(key)
          signal.removeEventListener("abort", cancel)
          resolve(outcome)
        }

        function cancel(): void {
          end("cancelled")
        }

        settlers.current.set(key, (loaded) => end(loaded ? "loaded" : "failed"))
        signal.addEventListener("abort", cancel)
        place(slot, { item, key, phase: "loading" })
      })
    }

    function bringForward(slot: 0 | 1): void {
      show((previous) => ({
        layers: raise(previous.layers, slot),
        resting: false,
      }))
    }

    async function prepare(): Promise<SlideSource | null> {
      const [albums, memories] = await Promise.all([
        sources.albums().catch(() => unavailablePhotos),
        sources.onThisDay().catch(() => unavailablePhotos),
      ])

      if (albums.state !== "ready") return null
      const seed = seedOf(day)
      const pages = dealPages(albums.albums, seed)

      if (pages.length === 0) return null
      const remembered = memory.recall()

      const normal = slideSource(
        pages,
        seed,
        (page) => sources.page(page).catch(() => unavailablePhotos),
        remembered?.day === day ? remembered.position : undefined
      )

      const preferred =
        memories.state === "ready" && "onThisDay" in memories
          ? memories.onThisDay.items
          : []

      let at = 0

      return {
        next: async () => preferred[at++] ?? normal.next(),
        position: normal.position,
      }
    }

    async function rest(): Promise<void> {
      show(() => ({ layers: [null, null], resting: true }))
      await wait(REST_MS, signal)
    }

    async function run(): Promise<void> {
      let source: SlideSource | null = null
      let slot: 0 | 1 = 0
      let misses = 0
      let shownAt: number | null = null

      while (!signal.aborted) {
        try {
          source ??= await prepare()
          const item = source === null ? null : await source.next()

          if (signal.aborted) return

          if (source === null || item === null) {
            source = null
            shownAt = null
            await rest()
            continue
          }

          const outcome = await load(slot, item)

          if (outcome === "cancelled") return

          if (outcome === "failed") {
            misses += 1

            if (misses < MISSES) continue
            misses = 0
            source = null
            shownAt = null
            await rest()
            continue
          }

          misses = 0

          if (shownAt !== null)
            await wait(Math.max(0, shownAt + DWELL_MS - Date.now()), signal)

          if (signal.aborted) return
          bringForward(slot)
          shownAt = Date.now()
          memory.keep({
            day: copenhagenDay(new Date()),
            position: source.position(),
          })
          await wait(FADE_MS, signal)
          slot = slot === 0 ? 1 : 0
        } catch {
          if (signal.aborted) return
          source = null
          shownAt = null
          await rest()
        }
      }
    }

    void run()

    return () => {
      stop.abort()
      setStage(blankStage)
    }
  }, [memory, sources, day])

  function settle(key: number, loaded: boolean): void {
    settlers.current.get(key)?.(loaded)
  }

  const front = layers.find((layer) => layer?.phase === "front")
  const year = front?.item.year

  return (
    <>
      <div
        data-slot="idle-stage"
        className="absolute inset-0"
        data-motion={still ? "still" : "drift"}
        data-resting={resting}
        aria-hidden="true"
      >
        <Slide layer={layers[0]} settle={settle} />
        <Slide layer={layers[1]} settle={settle} />
      </div>
      <IdleOverlay
        weather={sources.weather}
        yearsAgo={
          year === undefined ? undefined : Number(day.slice(0, 4)) - year
        }
      >
        {overlay}
      </IdleOverlay>
    </>
  )
}

const drifts = [
  "motion-safe:animate-idle-drift",
  "motion-safe:animate-idle-drift-back",
  "motion-safe:animate-idle-drift-across",
  "motion-safe:animate-idle-drift-across-back",
] as const

function Slide({
  layer,
  settle,
}: {
  layer: Layer | null
  settle: (key: number, loaded: boolean) => void
}): React.ReactNode {
  if (layer === null) return null

  return (
    <div
      data-slot="idle-slide"
      className="absolute inset-0 opacity-0 data-[front=true]:z-1 data-[front=true]:transition-opacity data-[front=true]:duration-idle-fade data-[front=true]:ease-app data-[shown=true]:opacity-100"
      data-front={layer.phase === "front"}
      data-shown={layer.phase !== "loading"}
    >
      <SlideImage
        key={layer.key}
        item={layer.item}
        drift={layer.phase === "loading" ? undefined : drifts[layer.key % 4]}
        settle={(loaded) => settle(layer.key, loaded)}
      />
    </div>
  )
}

function fitOf(image: HTMLImageElement): "cover" | "contain" {
  const photo = image.naturalWidth / Math.max(1, image.naturalHeight)
  const screen = window.innerWidth / Math.max(1, window.innerHeight)

  return Math.abs(Math.log(photo / screen)) < Math.log(1.25)
    ? "cover"
    : "contain"
}

function SlideImage({
  item,
  drift,
  settle,
}: {
  item: WallItem
  drift: string | undefined
  settle: (loaded: boolean) => void
}): React.JSX.Element {
  const [fit, setFit] = useState<"cover" | "contain">("contain")

  return (
    <img
      src={photoUrl(item.album, item.asset, "preview")}
      alt=""
      decoding="async"
      data-fit={fit}
      className={cn(
        "size-full object-contain data-[fit=cover]:object-cover",
        drift
      )}
      onLoad={(event) => {
        setFit(fitOf(event.currentTarget))
        settle(true)
      }}
      onError={() => settle(false)}
    />
  )
}
