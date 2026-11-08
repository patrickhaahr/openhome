import { useEffect, useRef, useState, useSyncExternalStore } from "react"
import { LiveWeather } from "@/components/live-weather"
import { timeFormat } from "@/components/live-clock"
import { degrees, WeatherGlyph } from "@/components/weather-overview"
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
import { useMinute } from "@/lib/use-minute"
import {
  unavailableWeather,
  weatherDescription,
  type WeatherRead,
} from "@/lib/weather"
import { getWeather } from "@/lib/weather.functions"

/** How long each photo stays before the next fades in. */
export const DWELL_MS = 12_000

/** The slow crossfade; the drift spans the dwell and both fades. */
export const FADE_MS = 2400

/** An unavailable library or network is tried again this much later, quietly. */
export const REST_MS = 60_000

/** Photos that fail in a row before the slideshow rests on the plain ground. */
const MISSES = 4

/** Where the slideshow reads from: Axum through the dashboard server, or a test's fakes. */
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

const dateFormat = new Intl.DateTimeFormat("en-GB", {
  weekday: "long",
  day: "numeric",
  month: "long",
  timeZone: "Europe/Copenhagen",
})

const reducedMotion = "(prefers-reduced-motion: reduce)"

function subscribeToMotion(onChange: () => void): () => void {
  const query = matchMedia(reducedMotion)
  query.addEventListener("change", onChange)

  return () => query.removeEventListener("change", onChange)
}

/** The household's motion choice; the server assumes stillness. */
export function useReducedMotion(): boolean {
  return useSyncExternalStore(
    subscribeToMotion,
    () => matchMedia(reducedMotion).matches,
    () => true
  )
}

/**
 * The Idle Slideshow: photos from the selected albums crossfading on the night ground, and the
 * overlay plate in the corner. It covers the dashboard; the provider owns waking.
 */
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
      className="idle dark"
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

/**
 * The tone plate in the corner: a large clock, the date and today's weather. `children` is the
 * slot for later idle content (the important-dates counter), set beneath them on the same plate.
 */
export function IdleOverlay({
  weather,
  children,
  yearsAgo,
}: {
  weather: IdleSources["weather"]
  children?: React.ReactNode
  yearsAgo?: number | undefined
}): React.JSX.Element {
  const [openedAt] = useState(() => new Date().toISOString())
  const now = useMinute(openedAt)

  return (
    <section className="idle-plate" aria-label="Now">
      <time
        className="idle-time num"
        dateTime={now.toISOString()}
        aria-label="Current time"
      >
        {timeFormat.format(now)}
      </time>
      <p className="idle-date">{dateFormat.format(now)}</p>
      <IdleWeather read={weather} />
      {yearsAgo === undefined ? null : (
        <p className="idle-photo-memory num">
          {yearsAgo} {yearsAgo === 1 ? "year" : "years"} ago today
        </p>
      )}
      {children === undefined || children === null ? null : (
        <div className="idle-extra">{children}</div>
      )}
    </section>
  )
}

/** Today's weather, read once on entering and kept fresh; missing weather simply isn't shown. */
function IdleWeather({
  read,
}: {
  read: IdleSources["weather"]
}): React.ReactNode {
  const [initial, setInitial] = useState<WeatherRead>()

  useEffect(() => {
    let current = true

    void read()
      .catch(() => unavailableWeather)
      .then((weather) => {
        if (current) setInitial(weather)
      })

    return () => {
      current = false
    }
  }, [read])

  if (initial === undefined) return null

  return (
    <LiveWeather initial={initial} readWeather={read}>
      {(weather) => {
        if (weather.state === "unavailable") return null
        const { current, days } = weather.forecast
        const today = days[0]

        return (
          <p className="idle-weather">
            <WeatherGlyph code={current?.weather_code ?? null} size={26} />
            <span className="idle-weather-now num" aria-hidden="true">
              {degrees(current?.temperature_c)}
            </span>
            {today === undefined ? null : (
              <span className="idle-weather-range num" aria-hidden="true">
                {degrees(today.high_c)} / {degrees(today.low_c)}
              </span>
            )}
            <span className="sr-only">
              {weatherDescription(current?.weather_code ?? null)},{" "}
              {degrees(current?.temperature_c)}
              {today === undefined
                ? ""
                : `, high ${degrees(today.high_c)}, low ${degrees(today.low_c)}`}
            </span>
          </p>
        )
      }}
    </LiveWeather>
  )
}

/**
 * One of the two image slots. A layer is loading beneath, fully in front, or behind the front
 * photo while it fades out (it has been on screen, so it keeps drifting).
 */
type Layer = {
  readonly item: WallItem
  readonly key: number
  readonly phase: "loading" | "front" | "behind"
}

type Layers = readonly [Layer | null, Layer | null]

/** What the screen shows: the two slots, and whether the plain ground is resting. */
type Stage = { readonly layers: Layers; readonly resting: boolean }

const blankStage: Stage = { layers: [null, null], resting: false }

/** How a photo load ended; a cancelled load is neither a hit nor a miss. */
type Loaded = "loaded" | "failed" | "cancelled"

/** `slot` comes to the front; a photo that was in front fades out behind it. */
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

/** Resolves when the time is up or the signal aborts, at once if it already has. */
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

/**
 * Two image slots and nothing more: the photo on screen and the next one preloading under it.
 * When the next has loaded and the dwell is over, it crossfades in and the old slot reloads.
 * An unavailable library or a run of broken photos rests on the plain ground and quietly tries
 * again later.
 *
 * Each effect run is the one owner of its own playback, and its `AbortSignal` is its
 * generation: once aborted, the run neither writes the screen nor the remembered position. The
 * local waits (an image load, the dwell, fade and rest timers) settle on the abort. A page read
 * does not: the browser's server-function call is neither cancelled nor bounded here, so an
 * old run can stay pending until that call settles, and its late result or failure is then
 * ignored. That matters because an old run's page read, image event or timer can still
 * complete after a newer run has taken the slots. (In production `memory` and `sources` are
 * stable and `day` remounts the slideshow, so a replacement run only follows waking; the
 * dependency list still allows one.) The sequential protocol stays one loop:
 * - dealing: `prepare` never rejects; it resolves with the albums, or with null for the ground,
 *   once its browser server-function calls settle. Those calls take no signal and are not
 *   bounded here, and an abort does not settle them. After `prepare()` settles, the run may
 *   await `source.next()` and may issue a page request (not when `prepare` gave null, nor
 *   when the next photo is an on-this-day one); it returns without drawing once it reaches
 *   the abort check or the cancellation-aware catch. Cancellation neither prevents that
 *   request nor settles the old run;
 * - page read (`source.next`): resolves, or rejects when the library is unreachable, and the
 *   loop rests. Upstream, Axum requests carry 7s/45s timeouts (source-supported, not an
 *   end-to-end guarantee: the browser's call to the dashboard server has no timeout or abort
 *   here). A cancelled run notices the abort only when the read settles;
 * - preload (`load`): ends on the image's load or error, or the abort. The 12s media timeout
 *   makes the server answer a 503 and so an error event, but the browser's own hop to the
 *   dashboard has no timeout here: a connection that never answers waits for the abort;
 * - dwell and fade (`wait`) end on the timer or the abort; rest likewise.
 */
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
  // Loads waiting on an image event; keys never repeat, so an old image cannot settle a new load.
  const settlers = useRef(new Map<number, (loaded: boolean) => void>())
  const keys = useRef(0)

  useEffect(() => {
    const stop = new AbortController()
    const { signal } = stop

    /** The only way a run changes the screen; a cancelled run changes nothing. */
    function show(change: (previous: Stage) => Stage): void {
      if (!signal.aborted) setStage(change)
    }

    function place(slot: 0 | 1, layer: Layer): void {
      show(({ layers: previous, resting: ground }) => ({
        layers: slot === 0 ? [layer, previous[1]] : [previous[0], layer],
        resting: ground,
      }))
    }

    /** Puts a photo in a slot, unseen, and ends when it has loaded, failed or the run is cancelled. */
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
          // The old photo stays beneath until the crossfade is over.
          await wait(FADE_MS, signal)
          slot = slot === 0 ? 1 : 0
        } catch {
          // A page read that fails after cancellation belongs to no one.
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
      // A replacement run does not inherit this run's photos: its slots start empty.
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
        className="idle-stage"
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

/** One slot; a new photo replaces only the image inside it. */
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
      className="idle-slide"
      data-front={layer.phase === "front"}
      data-shown={layer.phase !== "loading"}
      data-drift={layer.key % 4}
    >
      <SlideImage
        key={layer.key}
        item={layer.item}
        settle={(loaded) => settle(layer.key, loaded)}
      />
    </div>
  )
}

/** A photo shaped close to the screen fills it; any other is shown whole on the ground. */
function fitOf(image: HTMLImageElement): "cover" | "contain" {
  const photo = image.naturalWidth / Math.max(1, image.naturalHeight)
  const screen = window.innerWidth / Math.max(1, window.innerHeight)

  return Math.abs(Math.log(photo / screen)) < Math.log(1.25)
    ? "cover"
    : "contain"
}

function SlideImage({
  item,
  settle,
}: {
  item: WallItem
  settle: (loaded: boolean) => void
}): React.JSX.Element {
  const [fit, setFit] = useState<"cover" | "contain">("contain")

  return (
    <img
      src={photoUrl(item.album, item.asset, "preview")}
      alt=""
      decoding="async"
      data-fit={fit}
      onLoad={(event) => {
        setFit(fitOf(event.currentTarget))
        settle(true)
      }}
      onError={() => settle(false)}
    />
  )
}
