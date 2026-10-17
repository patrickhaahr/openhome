import { PAGE_SIZE, type Album, type WallItem, type WallRead } from "./photos"

/** How long the dashboard waits, untouched and unblocked, before the Idle Slideshow. */
export const DEFAULT_IDLE_SECONDS = 300

const MIN_IDLE_SECONDS = 10

const MAX_IDLE_SECONDS = 86_400

/** The server's idle choices; the browser never reads the environment itself. */
export type IdleSettings = {
  /** Null turns the Idle Slideshow off. */
  readonly timeoutMs: number | null
  /** Whether `?idle=now` may start the slideshow at once (synthetic previews only). */
  readonly preview: boolean
}

export const defaultIdleSettings: IdleSettings = {
  timeoutMs: DEFAULT_IDLE_SECONDS * 1000,
  preview: false,
}

/**
 * `IDLE_TIMEOUT_SECONDS` (whole seconds, 10 to 86400; 0 turns the slideshow off; anything
 * else keeps the 5-minute default) and `IDLE_PREVIEW=true`, which only a synthetic preview sets.
 */
export function parseIdleSettings(
  timeout: string | undefined,
  preview: string | undefined
): IdleSettings {
  const text = timeout?.trim() ?? ""
  const seconds = /^\d+$/.test(text) ? Number(text) : Number.NaN

  const timeoutMs =
    seconds === 0
      ? null
      : Number.isInteger(seconds) &&
          seconds >= MIN_IDLE_SECONDS &&
          seconds <= MAX_IDLE_SECONDS
        ? seconds * 1000
        : DEFAULT_IDLE_SECONDS * 1000

  return { timeoutMs, preview: preview?.trim() === "true" }
}

/** `?idle=now` asks for the slideshow at once; honoured only when the server allows previews. */
export function wantsIdleNow(settings: IdleSettings, search: string): boolean {
  return settings.preview && new URLSearchParams(search).get("idle") === "now"
}

/** A small, fast, seeded generator (mulberry32): the same seed deals the same order. */
export function seeded(seed: number): () => number {
  let state = seed >>> 0

  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let mixed = Math.imul(state ^ (state >>> 15), 1 | state)
    mixed ^= mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)

    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4_294_967_296
  }
}

/** A string as a 32-bit seed (FNV-1a). */
export function seedOf(text: string): number {
  let hash = 0x811c9dc5

  for (const char of text) {
    hash ^= char.codePointAt(0) ?? 0
    hash = Math.imul(hash, 0x01000193)
  }

  return hash >>> 0
}

/** A Fisher-Yates shuffle of a copy, in the seed's order. */
export function shuffled<T>(items: readonly T[], seed: number): T[] {
  const random = seeded(seed)
  const copy = [...items]

  for (let at = copy.length - 1; at > 0; at -= 1) {
    const other = Math.floor(random() * (at + 1))
    const held = copy[at]
    const swapped = copy[other]

    if (held === undefined || swapped === undefined) continue
    copy[at] = swapped
    copy[other] = held
  }

  return copy
}

/** One page of one selected album, read only when the slideshow reaches it. */
export type DeckPage = { readonly album: string; readonly page: number }

/**
 * Every page of every available album, dealt in the seed's order so albums interleave.
 * Only page numbers are held; photo identities are read a page at a time.
 */
export function dealPages(albums: readonly Album[], seed: number): DeckPage[] {
  const pages = albums.flatMap((album) =>
    album.missing
      ? []
      : Array.from(
          { length: Math.ceil(album.photo_count / PAGE_SIZE) },
          (_, page) => ({ album: album.id, page })
        )
  )

  return shuffled(pages, seed)
}

/** Where the slideshow stands, so the next idle spell carries on rather than starting over. */
export type DeckPosition = { readonly page: number; readonly photo: number }

type Remembered = { readonly day: string; readonly position: DeckPosition }

/** The slideshow's place, kept while the dashboard is awake so the next spell carries on. */
export type SlideMemory = {
  readonly recall: () => Remembered | null
  readonly keep: (remembered: Remembered) => void
}

export function slideMemory(): SlideMemory {
  let remembered: Remembered | null = null

  return {
    recall: () => remembered,
    keep: (next) => {
      remembered = next
    },
  }
}

export type SlideSource = {
  /** The next photo, reading the next page when one runs out; null when there is none. */
  readonly next: () => Promise<WallItem | null>
  readonly position: () => DeckPosition
}

/**
 * Photos in a random but stable order: pages in the dealt order, each page's photos shuffled
 * by the same seed. At most one page of identities is held at a time. A failed page read
 * rejects, so the caller can rest and try again later from the same place.
 */
export function slideSource(
  pages: readonly DeckPage[],
  seed: number,
  readPage: (page: DeckPage) => Promise<WallRead>,
  start: DeckPosition = { page: 0, photo: 0 }
): SlideSource {
  let pageAt = pages.length === 0 ? 0 : start.page % pages.length
  let photoAt = start.photo
  let held: WallItem[] | null = null

  async function load(): Promise<WallItem[]> {
    const page = pages[pageAt]

    if (page === undefined) return []
    const read = await readPage(page)

    if (read.state === "ready")
      return shuffled(
        read.wall.items,
        seed ^ seedOf(`${page.album}:${page.page}`)
      )

    // A removed album or page is skipped; an unreachable library is tried again later.
    if (read.reason === "not-found" || read.reason === "invalid-page") return []

    throw new Error("Photo page unavailable")
  }

  return {
    next: async () => {
      if (pages.length === 0) return null

      // Every page may be empty (photos removed since the albums were read): one lap at most.
      for (let tries = 0; tries <= pages.length; tries += 1) {
        held ??= await load()
        const item = held[photoAt]

        if (item !== undefined) {
          photoAt += 1

          return item
        }

        held = null
        photoAt = 0
        pageAt = (pageAt + 1) % pages.length
      }

      return null
    },
    position: () => ({ page: pageAt, photo: photoAt }),
  }
}

/** Today in Copenhagen, the seed's day: the order is stable all day and fresh tomorrow. */
export function copenhagenDay(at: Date): string {
  return new Intl.DateTimeFormat("en-CA", {
    timeZone: "Europe/Copenhagen",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(at)
}
