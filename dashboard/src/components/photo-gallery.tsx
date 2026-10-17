import { useCallback, useEffect, useEffectEvent, useRef, useState } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { ArrowLeftIcon } from "@phosphor-icons/react/dist/csr/ArrowLeft"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { ImageBrokenIcon } from "@phosphor-icons/react/dist/csr/ImageBroken"
import { ImagesIcon } from "@phosphor-icons/react/dist/csr/Images"
import { StackIcon } from "@phosphor-icons/react/dist/csr/Stack"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { useIdleBlock } from "@/components/idle-block"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog"
import { formatWhole } from "@/lib/number-format"
import {
  ALL_PHOTOS,
  ON_THIS_DAY,
  formatMonthDay,
  librarySpan,
  monthSpan,
  photoUrl,
  unavailablePhotos,
  type Album,
  type AlbumRead,
  type OnThisDayResult,
  type Wall,
  type WallItem,
  type WallRead,
} from "@/lib/photos"

type GalleryProps = {
  albums: AlbumRead
  /** The open album id, all photos, or null for the albums. */
  view: string | null
  wall: WallRead | null
  /** Today's photos from earlier years, offered with the albums. */
  onThisDay?: OnThisDayResult | null
  /** The open photo, from the URL. */
  photo: string | undefined
  open: (album: string | undefined) => void
  show: (photo: string | undefined) => void
  more: (album: string, page: number) => Promise<WallRead>
  retry: () => Promise<void>
}

/**
 * Albums are cover plates; an album, or all of them merged, is a justified wall of photos that
 * keeps loading as it scrolls; a photo opens full screen on black. Names and counts that the
 * pictures already say stay for assistive technology.
 */
export function PhotoGallery({
  albums,
  view,
  wall,
  onThisDay = null,
  photo,
  open,
  show,
  more,
  retry,
}: GalleryProps): React.JSX.Element {
  if (albums.state !== "ready")
    return albums.reason === "missing-configuration" ? (
      <Unavailable
        title="Photo setup needed"
        detail="No photo library is connected here. Choose albums in the private OpenHome API configuration."
        retry={retry}
      />
    ) : (
      <Unavailable
        title="Photo library unavailable"
        detail="Check Connection or try again. Your other dashboard features are still available."
        retry={retry}
      />
    )

  if (view === null || wall === null)
    return (
      <AlbumIndex albums={albums.albums} onThisDay={onThisDay} open={open} />
    )

  const available = albums.albums.filter((album) => !album.missing)
  const source = albums.albums.find((album) => album.id === view)
  const all = view === ALL_PHOTOS
  const today = view === ON_THIS_DAY
  const back = albums.albums.length > 1 ? () => open(undefined) : undefined

  const head = {
    title: today
      ? "On this day"
      : all
        ? "All photos"
        : (source?.name ?? "Album"),
    span: today
      ? null
      : all
        ? librarySpan(available)
        : source === undefined
          ? null
          : monthSpan(source.first_month, source.last_month),
    back,
  }

  if (wall.state !== "ready")
    return (
      <>
        <WallHead {...head} total={null} />
        <Unavailable
          title="Album unavailable"
          detail={
            wall.reason === "not-found"
              ? "This album was removed or is not selected for the dashboard. Choose an available album."
              : "Choose an available album or try again."
          }
          retry={retry}
        />
      </>
    )

  return (
    <PhotoWall
      key={view}
      view={view}
      first={wall.wall}
      head={head}
      names={new Map(albums.albums.map((album) => [album.id, album.name]))}
      photo={photo}
      show={show}
      more={more}
    />
  )
}

function Unavailable({
  title,
  detail,
  retry,
}: {
  title: string
  detail: string
  retry: () => Promise<void>
}): React.JSX.Element {
  const [retrying, setRetrying] = useState(false)
  const [failed, setFailed] = useState(false)
  const label = retrying ? "Trying again…" : "Try again"

  async function again(): Promise<void> {
    setRetrying(true)
    setFailed(false)

    try {
      await retry()
    } catch {
      setFailed(true)
    } finally {
      setRetrying(false)
    }
  }

  return (
    <div className="photo-empty" role="status">
      <ImageBrokenIcon size={56} weight="light" aria-hidden="true" />
      <h2>{title}</h2>
      <p className="sr-only">{detail}</p>
      <Button
        variant="outline"
        size="key-icon"
        disabled={retrying}
        data-spinning={retrying}
        title={label}
        onClick={() => void again()}
      >
        <ArrowClockwiseIcon size={20} aria-hidden="true" />
        <span className="sr-only">{label}</span>
      </Button>
      {failed ? (
        <p className="photo-failed">
          <span className="sr-only">Still unavailable. Try again later.</span>
        </p>
      ) : null}
    </div>
  )
}

/** The count glyph and figure, and the months, under an album's name. */
function AlbumMeta({
  count,
  span,
  unit,
}: {
  count: number
  span: string | null
  unit: "photos" | "albums"
}): React.JSX.Element {
  const Icon = unit === "photos" ? ImagesIcon : StackIcon

  return (
    <span className="photo-album-meta">
      <span className="photo-album-count">
        <Icon size={18} aria-hidden="true" />
        <span className="num">{formatWhole(count)}</span>
        <span className="sr-only"> {unit}</span>
      </span>
      {span === null ? null : <span className="num">{span}</span>}
    </span>
  )
}

function AlbumIndex({
  albums,
  onThisDay,
  open,
}: {
  albums: Album[]
  onThisDay: OnThisDayResult | null
  open: (album: string) => void
}): React.JSX.Element {
  const available = albums.filter((album) => !album.missing)

  const covers = available.flatMap((album) =>
    album.cover_id === null ? [] : [{ album: album.id, asset: album.cover_id }]
  )

  const today =
    onThisDay?.state === "ready" && onThisDay.onThisDay.total > 0
      ? onThisDay.onThisDay
      : null

  return (
    <>
      <h2 className="sr-only">Albums</h2>
      <ul className="photo-albums">
        {available.length > 1 ? (
          <li className="photo-album" data-kind="all">
            <button type="button" onClick={() => open(ALL_PHOTOS)}>
              <span
                className="photo-album-cover"
                data-count={Math.min(covers.length, 4)}
              >
                {covers.slice(0, 4).map((cover) => (
                  <Cover key={cover.album} {...cover} />
                ))}
              </span>
              <span className="photo-album-name">All photos</span>
              <AlbumMeta
                count={available.length}
                span={librarySpan(available)}
                unit="albums"
              />
            </button>
          </li>
        ) : null}
        {today === null ? null : (
          <li className="photo-album" data-kind="on-this-day">
            <button type="button" onClick={() => open(ON_THIS_DAY)}>
              <span
                className="photo-album-cover"
                data-count={Math.min(today.items.length, 4)}
              >
                {today.items.slice(0, 4).map((cover) => (
                  <Cover
                    key={cover.asset}
                    album={cover.album}
                    asset={cover.asset}
                  />
                ))}
              </span>
              <span className="photo-album-name">On this day</span>
              <AlbumMeta
                count={today.total}
                span={formatMonthDay(today.date)}
                unit="photos"
              />
            </button>
          </li>
        )}
        {albums.map((album) => (
          <li key={album.id} className="photo-album">
            <button
              type="button"
              disabled={album.missing}
              onClick={() => open(album.id)}
            >
              <span className="photo-album-cover" data-count="1">
                {album.cover_id === null ? (
                  <span className="photo-album-blank">
                    {album.missing ? (
                      <ImageBrokenIcon
                        size={40}
                        weight="light"
                        aria-hidden="true"
                      />
                    ) : (
                      <ImagesIcon size={40} weight="light" aria-hidden="true" />
                    )}
                  </span>
                ) : (
                  <Cover album={album.id} asset={album.cover_id} />
                )}
              </span>
              <span className="photo-album-name">{album.name}</span>
              {album.missing ? (
                <span className="sr-only">
                  Removed from the photo library or no longer shared
                </span>
              ) : (
                <AlbumMeta
                  count={album.photo_count}
                  span={monthSpan(album.first_month, album.last_month)}
                  unit="photos"
                />
              )}
            </button>
          </li>
        ))}
      </ul>
    </>
  )
}

/** A decorative cover: the album's name already names the plate. */
function Cover({ album, asset }: WallItem): React.JSX.Element {
  const [failed, setFailed] = useState(false)

  const imageRef = useCallback((image: HTMLImageElement | null) => {
    if (image?.complete === true && image.naturalWidth === 0) setFailed(true)
  }, [])

  return failed ? (
    <span className="photo-album-blank">
      <ImageBrokenIcon size={32} weight="light" aria-hidden="true" />
    </span>
  ) : (
    <img
      ref={imageRef}
      src={photoUrl(album, asset, "thumbnail")}
      alt=""
      loading="lazy"
      decoding="async"
      onError={() => setFailed(true)}
    />
  )
}

type Head = {
  title: string
  span: string | null
  back: (() => void) | undefined
}

function WallHead({
  title,
  span,
  back,
  total,
}: Head & { total: number | null }): React.JSX.Element {
  return (
    <header className="photo-head">
      {back === undefined ? null : (
        <Button
          variant="outline"
          size="key-icon"
          title="All albums"
          onClick={back}
        >
          <ArrowLeftIcon size={20} aria-hidden="true" />
          <span className="sr-only">All albums</span>
        </Button>
      )}
      <h2 className="photo-head-title">{title}</h2>
      {total === null ? null : (
        <AlbumMeta count={total} span={span} unit="photos" />
      )}
    </header>
  )
}

// Thumbnails tell their own frame once loaded; until then a tile is a 3:2 landscape.
const DEFAULT_RATIO = 1.5

const ratios = new Map<string, number>()

function ratioOf(width: number, height: number): number {
  return Math.min(2.4, Math.max(0.5, width / height))
}

function PhotoWall({
  view,
  first,
  head,
  names,
  photo,
  show,
  more,
}: {
  view: string
  first: Wall
  head: Head
  names: Map<string, string>
  photo: string | undefined
  show: GalleryProps["show"]
  more: GalleryProps["more"]
}): React.JSX.Element {
  const [items, setItems] = useState(first.items)
  const [next, setNext] = useState(first.next)
  const [loading, setLoading] = useState(false)
  const [failed, setFailed] = useState(false)
  const [announcement, setAnnouncement] = useState("")
  const busy = useRef(false)
  const end = useRef<HTMLDivElement>(null)
  const tiles = useRef(new Map<string, HTMLButtonElement>())
  const shown = useRef<string | undefined>(undefined)
  const swipe = useRef<{ x: number; y: number } | null>(null)

  const index =
    photo === undefined ? -1 : items.findIndex((item) => item.asset === photo)

  const current = index === -1 ? undefined : items[index]

  // Someone is looking at a photo; the slideshow waits until it is closed.
  useIdleBlock(current !== undefined)

  // Focus returns to the last photo shown, even after stepping away from the one opened.
  useEffect(() => {
    if (current !== undefined) shown.current = current.asset
  }, [current])

  async function load(advance: boolean): Promise<void> {
    if (busy.current || next === null) return
    busy.current = true
    setLoading(true)
    setFailed(false)
    const read = await more(view, next).catch(() => unavailablePhotos)
    busy.current = false
    setLoading(false)

    if (read.state !== "ready") {
      setFailed(true)
      setAnnouncement("Could not load more photos.")

      return
    }

    const known = new Set(items.map((item) => item.asset))
    const fresh = read.wall.items.filter((item) => !known.has(item.asset))
    setItems([...items, ...fresh])
    setNext(read.wall.next)
    setAnnouncement(`${fresh.length} more photos loaded.`)

    if (advance && fresh[0] !== undefined) show(fresh[0].asset)
  }

  const nearEnd = useEffectEvent(() => void load(false))

  // Observed again after every page, so a wall still short of the screen keeps filling.
  useEffect(() => {
    const target = end.current

    if (
      target === null ||
      next === null ||
      failed ||
      !("IntersectionObserver" in window)
    )
      return

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) nearEnd()
      },
      { rootMargin: "0px 0px 1200px 0px" }
    )

    observer.observe(target)

    return () => observer.disconnect()
  }, [items.length, next, failed])

  function go(direction: -1 | 1): void {
    const target = items[index + direction]

    if (target !== undefined) show(target.asset)
    else if (direction === 1 && next !== null) void load(true)
  }

  const position = index + 1

  const albumName = (item: WallItem): string =>
    `${names.get(item.album) ?? "album"}${item.year === undefined ? "" : `, ${item.year}`}`

  return (
    <>
      <WallHead {...head} total={first.total} />
      {items.length === 0 ? (
        <div className="photo-empty">
          <ImagesIcon size={56} weight="light" aria-hidden="true" />
          <h3>No photos here yet</h3>
          <p className="sr-only">This album has no available still photos.</p>
        </div>
      ) : (
        <ul className="photo-wall">
          {items.map((item, at) => (
            <Tile
              key={item.asset}
              item={item}
              label={`Open photo ${at + 1}`}
              alt={`Photo ${at + 1} from ${albumName(item)}`}
              register={(element) => {
                if (element === null) tiles.current.delete(item.asset)
                else tiles.current.set(item.asset, element)
              }}
              open={() => show(item.asset)}
            />
          ))}
        </ul>
      )}
      <div ref={end} className="photo-more">
        {next === null ? null : (
          <Button
            variant="outline"
            size="lg"
            disabled={loading}
            onClick={() => void load(false)}
          >
            {loading ? "Loading photos…" : failed ? "Try again" : "Load more"}
            {failed ? (
              <span className="photo-failed" aria-hidden="true" />
            ) : null}
          </Button>
        )}
      </div>
      <p className="sr-only" role="status">
        {announcement}
      </p>
      <Dialog
        open={current !== undefined}
        onOpenChange={(isOpen) => {
          if (!isOpen) show(undefined)
        }}
      >
        <DialogContent
          variant="fullscreen"
          showCloseButton={false}
          finalFocus={() =>
            shown.current === undefined
              ? true
              : (tiles.current.get(shown.current) ?? true)
          }
          onKeyDown={(event) => {
            if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
              event.preventDefault()
              go(event.key === "ArrowRight" ? 1 : -1)
            }
          }}
        >
          <div className="photo-viewer">
            <div className="sr-only">
              <DialogTitle>{head.title}</DialogTitle>
              <DialogDescription>
                Photo {position} of {first.total}
              </DialogDescription>
            </div>
            <div
              className="photo-stage"
              onPointerDown={(event) => {
                swipe.current =
                  event.pointerType === "mouse"
                    ? null
                    : { x: event.clientX, y: event.clientY }
              }}
              onPointerCancel={() => {
                swipe.current = null
              }}
              onPointerUp={(event) => {
                const start = swipe.current
                swipe.current = null

                if (start === null) return
                const dx = event.clientX - start.x
                const dy = event.clientY - start.y

                if (Math.abs(dx) > 48 && Math.abs(dx) > Math.abs(dy) * 1.5)
                  go(dx < 0 ? 1 : -1)
              }}
            >
              {current === undefined ? null : (
                <PhotoImage
                  key={current.asset}
                  src={photoUrl(current.album, current.asset, "preview")}
                  alt={`Photo ${position} from ${albumName(current)}`}
                />
              )}
            </div>
            <div className="photo-viewer-bar">
              <span className="photo-viewer-where" aria-hidden="true">
                <span className="num">
                  {position} / {formatWhole(first.total)}
                </span>
                {view === ALL_PHOTOS && current !== undefined ? (
                  <span>{albumName(current)}</span>
                ) : null}
                {current?.year === undefined ? null : (
                  <span className="num">{current.year}</span>
                )}
              </span>
              <DialogClose
                render={<Button variant="outline" size="key-icon" />}
                title="Close photo"
              >
                <XIcon size={22} aria-hidden="true" />
                <span className="sr-only">Close photo</span>
              </DialogClose>
            </div>
            <div className="photo-step" data-side="previous">
              <Button
                variant="outline"
                size="key-icon"
                disabled={index <= 0}
                title="Previous photo"
                onClick={() => go(-1)}
              >
                <CaretLeftIcon size={26} aria-hidden="true" />
                <span className="sr-only">← Previous</span>
              </Button>
            </div>
            <div className="photo-step" data-side="next">
              <Button
                variant="outline"
                size="key-icon"
                disabled={index === items.length - 1 && next === null}
                title="Next photo"
                onClick={() => go(1)}
              >
                <CaretRightIcon size={26} aria-hidden="true" />
                <span className="sr-only">Next →</span>
              </Button>
            </div>
            <span className="sr-only" aria-live="polite">
              {position} / {first.total}
            </span>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}

function Tile({
  item,
  label,
  alt,
  register,
  open,
}: {
  item: WallItem
  label: string
  alt: string
  register: (element: HTMLButtonElement | null) => void
  open: () => void
}): React.JSX.Element {
  const [ratio, setRatio] = useState(
    () => ratios.get(item.asset) ?? DEFAULT_RATIO
  )

  return (
    <li
      className="photo-cell"
      // SAFETY: CSSProperties omits custom properties; --ratio is the photo's width over height.
      style={{ "--ratio": ratio } as React.CSSProperties}
    >
      <button
        ref={register}
        type="button"
        className="photo-tile"
        aria-label={label}
        onClick={open}
      >
        <PhotoImage
          src={photoUrl(item.album, item.asset, "thumbnail")}
          alt={alt}
          lazy
          measure={(width, height) => {
            const value = ratioOf(width, height)
            ratios.set(item.asset, value)
            setRatio(value)
          }}
        />
      </button>
    </li>
  )
}

function PhotoImage({
  src,
  alt,
  lazy = false,
  measure,
}: {
  src: string
  alt: string
  lazy?: boolean
  measure?: (width: number, height: number) => void
}): React.JSX.Element {
  const [state, setState] = useState<"loading" | "loaded" | "failed">("loading")
  const [attempt, setAttempt] = useState(0)

  const settle = useEffectEvent((image: HTMLImageElement) => {
    if (image.naturalWidth > 0) {
      setState("loaded")
      measure?.(image.naturalWidth, image.naturalHeight)
    } else setState("failed")
  })

  const imageRef = (image: HTMLImageElement | null): void => {
    // SSR thumbnails can finish before React attaches its load/error listeners.
    if (image !== null && image.complete) settle(image)
  }

  return (
    <div className="photo-image" data-state={state}>
      {state === "failed" ? (
        <div className="photo-image-status" role="status">
          <ImageBrokenIcon
            size={lazy ? 28 : 48}
            weight="light"
            aria-hidden="true"
          />
          <span className="sr-only">Photo unavailable</span>
          {lazy ? null : (
            <Button
              variant="outline"
              size="key-icon"
              title="Retry photo"
              onClick={() => {
                setAttempt((value) => value + 1)
                setState("loading")
              }}
            >
              <ArrowClockwiseIcon size={20} aria-hidden="true" />
              <span className="sr-only">Retry photo</span>
            </Button>
          )}
        </div>
      ) : (
        <>
          <img
            key={attempt}
            ref={imageRef}
            src={`${src}${attempt ? `?retry=${attempt}` : ""}`}
            alt={alt}
            loading={lazy ? "lazy" : "eager"}
            decoding="async"
            onLoad={(event) => settle(event.currentTarget)}
            onError={() => setState("failed")}
          />
          {state === "loading" ? (
            <span className="photo-image-status">
              <span className="sr-only">Loading photo…</span>
            </span>
          ) : null}
        </>
      )}
    </div>
  )
}
