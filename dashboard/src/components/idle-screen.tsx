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

/** One of the two image slots: the photo showing, or the next one loading beneath it. */
type Layer = {
  readonly item: WallItem
  readonly key: number
  /** Fully faded in; the other slot is free to load. */
  readonly front: boolean
  /** Has been on screen, so it keeps drifting while it fades out. */
  readonly shown: boolean
}

type Layers = readonly [Layer | null, Layer | null]

function wait(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, ms)

    signal.addEventListener("abort", () => {
      clearTimeout(timer)
      resolve()
    })
  })
}

/**
 * Two image slots and nothing more: the photo on screen and the next one preloading under it.
 * When the next has loaded and the dwell is over, it crossfades in and the old slot reloads.
 * An unavailable library or a run of broken photos rests on the plain ground and quietly tries
 * again later.
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
  const [layers, setLayers] = useState<Layers>([null, null])
  const [resting, setResting] = useState(false)
  const settlers = useRef(new Map<number, (loaded: boolean) => void>())

  useEffect(() => {
    const stop = new AbortController()
    const { signal } = stop
    let counter = 0

    function place(slot: 0 | 1, layer: Layer | null): void {
      setLayers((previous) =>
        slot === 0 ? [layer, previous[1]] : [previous[0], layer]
      )
    }

    /** Puts a photo in a slot, unseen, and settles when it has loaded or failed. */
    function load(slot: 0 | 1, item: WallItem): Promise<boolean> {
      counter += 1
      const key = counter

      return new Promise((resolve) => {
        settlers.current.set(key, resolve)
        signal.addEventListener("abort", () => resolve(false))
        place(slot, { item, key, front: false, shown: false })
      })
    }

    function bringForward(slot: 0 | 1): void {
      setLayers((previous) => {
        const [first, second] = previous

        const raise = (layer: Layer | null, own: boolean): Layer | null =>
          layer === null
            ? null
            : { ...layer, front: own, shown: layer.shown || own }

        return [raise(first, slot === 0), raise(second, slot === 1)]
      })
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
      setLayers([null, null])
      setResting(true)
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

          if (!(await load(slot, item))) {
            if (signal.aborted) return
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
          setResting(false)
          shownAt = Date.now()
          memory.keep({
            day: copenhagenDay(new Date()),
            position: source.position(),
          })
          // The old photo stays beneath until the crossfade is over.
          await wait(FADE_MS, signal)
          slot = slot === 0 ? 1 : 0
        } catch {
          source = null
          shownAt = null
          await rest()
        }
      }
    }

    void run()

    return () => stop.abort()
  }, [memory, sources, day])

  function settle(key: number, loaded: boolean): void {
    settlers.current.get(key)?.(loaded)
    settlers.current.delete(key)
  }

  const front = layers.find((layer) => layer?.front === true)
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
      data-front={layer.front}
      data-shown={layer.shown}
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
