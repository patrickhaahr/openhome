import { useEffect, useEffectEvent, useRef, useState } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { ArrowLeftIcon } from "@phosphor-icons/react/dist/csr/ArrowLeft"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { ImageBrokenIcon } from "@phosphor-icons/react/dist/csr/ImageBroken"
import { ImagesIcon } from "@phosphor-icons/react/dist/csr/Images"
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
  librarySpan,
  monthSpan,
  photoUrl,
  unavailablePhotos,
  type AlbumRead,
  type OnThisDayResult,
  type Wall,
  type WallItem,
  type WallRead,
} from "@/lib/photos"
import { Tile, PhotoImage } from "@/components/photo-tile"
import { AlbumMeta, AlbumIndex } from "@/components/photo-albums"

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

type PageRequest =
  | { state: "idle" }
  | { state: "loading"; cursor: number }
  | { state: "failed"; cursor: number }

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
  const [request, setRequest] = useState<PageRequest>({ state: "idle" })
  const [announcement, setAnnouncement] = useState("")
  // Identifies the one request allowed to settle; cleared on unmount so a late page is ignored.
  const flight = useRef<symbol | null>(null)
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

  const loading = request.state === "loading"
  const failed = request.state === "failed"

  useEffect(
    () => () => {
      flight.current = null
    },
    []
  )

  // The only transition owner: one read at a time, a retry asks for the failed cursor again.
  async function load(advance: boolean): Promise<void> {
    if (flight.current !== null || next === null) return
    const cursor = request.state === "failed" ? request.cursor : next
    const token = Symbol("page")
    flight.current = token
    setRequest({ state: "loading", cursor })
    const read = await more(view, cursor).catch(() => unavailablePhotos)

    if (flight.current !== token) return
    flight.current = null

    if (read.state !== "ready") {
      setRequest({ state: "failed", cursor })
      setAnnouncement("Could not load more photos.")

      return
    }

    setRequest({ state: "idle" })

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
