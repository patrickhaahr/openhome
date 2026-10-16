import { useCallback, useState } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { ArrowLeftIcon } from "@phosphor-icons/react/dist/csr/ArrowLeft"
import { ArrowRightIcon } from "@phosphor-icons/react/dist/csr/ArrowRight"
import { ImageBrokenIcon } from "@phosphor-icons/react/dist/csr/ImageBroken"
import { ImagesIcon } from "@phosphor-icons/react/dist/csr/Images"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogDescription,
  DialogTrigger,
  DialogClose,
} from "@/components/ui/dialog"
import { photoUrl, type AlbumRead, type PhotoRead } from "@/lib/photos"

type GalleryProps = {
  albums: AlbumRead
  page: PhotoRead | null
  selectedAlbum: string | null
  select: (album: string, page: number) => void
  refresh: () => Promise<void>
}

/**
 * The album is the page: a mosaic, an album switch only when there is a choice, and a
 * full-screen viewer. Names and counts stay for assistive technology.
 */
export function PhotoGallery({
  albums,
  page,
  selectedAlbum,
  select,
  refresh,
}: GalleryProps): React.JSX.Element {
  const [retrying, setRetrying] = useState(false)
  const [retryFailed, setRetryFailed] = useState(false)
  const [revision, setRevision] = useState(0)

  async function retry(): Promise<void> {
    setRetrying(true)
    setRetryFailed(false)

    try {
      await refresh()
      setRevision((value) => value + 1)
    } catch {
      setRetryFailed(true)
    } finally {
      setRetrying(false)
    }
  }

  const retryLabel = retrying ? "Refreshing…" : "Try again"

  const retryButton = (
    <Button
      variant="outline"
      size="key-icon"
      disabled={retrying}
      data-spinning={retrying}
      title={retryLabel}
      onClick={() => void retry()}
    >
      <ArrowClockwiseIcon size={20} aria-hidden="true" />
      <span className="sr-only">{retryLabel}</span>
    </Button>
  )

  const failed = retryFailed ? (
    <p className="photo-failed" data-failed="true" role="status">
      <span className="sr-only">Could not refresh. Try again.</span>
    </p>
  ) : null

  if (albums.state !== "ready")
    return (
      <div className="photo-empty" role="status">
        <ImagesIcon size={56} weight="light" aria-hidden="true" />
        <h2>
          {albums.reason === "missing-configuration"
            ? "Photo setup needed"
            : "Photo library unavailable"}
        </h2>
        <p className="sr-only">
          {albums.reason === "missing-configuration"
            ? "No photo library is connected here. Choose albums in the private OpenHome API configuration."
            : "Check Connection or try again. Your other dashboard features are still available."}
        </p>
        {retryButton}
        {failed}
      </div>
    )

  return (
    <>
      <div className="photo-toolbar">
        {albums.albums.length > 1 ? (
          <nav className="photo-albums" aria-label="Photo albums">
            {albums.albums.map((album) => (
              <Button
                key={album.id}
                variant={selectedAlbum === album.id ? "default" : "ghost"}
                disabled={album.missing}
                aria-pressed={selectedAlbum === album.id}
                onClick={() => select(album.id, 0)}
              >
                {album.name}
                <span className="sr-only">
                  {album.missing ? "Missing" : `${album.photo_count} photos`}
                </span>
              </Button>
            ))}
          </nav>
        ) : null}
        <Button
          variant="ghost"
          size="key-icon"
          disabled={retrying}
          data-spinning={retrying}
          title={retrying ? "Refreshing…" : "Refresh photos"}
          onClick={() => void retry()}
        >
          <ArrowClockwiseIcon size={20} aria-hidden="true" />
          <span className="sr-only">
            {retrying ? "Refreshing…" : "Refresh photos"}
          </span>
        </Button>
      </div>
      {page?.state === "ready" ? (
        <AlbumPhotos
          key={`${page.photos.album.id}/${page.photos.page}/${revision}`}
          photos={page.photos}
          select={select}
        />
      ) : (
        <div className="photo-empty" role="status">
          <ImageBrokenIcon size={56} weight="light" aria-hidden="true" />
          <h2>
            {page?.reason === "invalid-page"
              ? "This page is unavailable"
              : "Album unavailable"}
          </h2>
          <p className="sr-only">
            {page?.reason === "not-found"
              ? "This album was removed or is not selected for the dashboard. Choose an available album."
              : "Choose an available album or try again."}
          </p>
          {page?.reason === "invalid-page" ? (
            <Button
              size="lg"
              onClick={() => {
                if (selectedAlbum !== null) select(selectedAlbum, 0)
              }}
            >
              First page
            </Button>
          ) : null}
          {retryButton}
        </div>
      )}
      {failed}
    </>
  )
}

function AlbumPhotos({
  photos,
  select,
}: {
  photos: Extract<PhotoRead, { state: "ready" }>["photos"]
  select: GalleryProps["select"]
}): React.JSX.Element {
  const [current, setCurrent] = useState(0)
  const [open, setOpen] = useState(false)
  const selected = photos.assets[current]
  const position = photos.page * 60 + current + 1

  const go = (direction: -1 | 1): void => {
    const index = current + direction

    if (index >= 0 && index < photos.assets.length) setCurrent(index)
  }

  return (
    <>
      <div className="sr-only">
        <h2>{photos.album.name}</h2>
        <p>
          {photos.album.photo_count} photos · Page {photos.page + 1}
        </p>
      </div>
      {photos.assets.length === 0 ? (
        <div className="photo-empty">
          <ImagesIcon size={56} weight="light" aria-hidden="true" />
          <h3>No photos here yet</h3>
          <p className="sr-only">This album has no available still photos.</p>
        </div>
      ) : (
        <Dialog open={open} onOpenChange={setOpen}>
          <div className="photo-grid">
            {photos.assets.map((id, index) => (
              <DialogTrigger
                key={id}
                render={<button className="photo-tile" type="button" />}
                aria-label={`Open photo ${photos.page * 60 + index + 1}`}
                onClick={() => setCurrent(index)}
              >
                <PhotoImage
                  key={id}
                  src={photoUrl(photos.album.id, id, "thumbnail")}
                  alt={`Photo ${photos.page * 60 + index + 1} from ${photos.album.name}`}
                  lazy
                />
              </DialogTrigger>
            ))}
          </div>
          <DialogContent
            variant="fullscreen"
            showCloseButton={false}
            onKeyDown={(event) => {
              if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
                event.preventDefault()
                go(event.key === "ArrowRight" ? 1 : -1)
              }
            }}
          >
            <header className="photo-viewer-header">
              <div className="sr-only">
                <DialogTitle>{photos.album.name}</DialogTitle>
                <DialogDescription>
                  Photo {position} of {photos.album.photo_count}
                </DialogDescription>
              </div>
              <span className="photo-viewer-count" aria-hidden="true">
                {position} / {photos.album.photo_count}
              </span>
              <DialogClose
                render={<Button variant="outline" size="key-icon" />}
                title="Close photo"
              >
                <XIcon size={20} aria-hidden="true" />
                <span className="sr-only">Close photo</span>
              </DialogClose>
            </header>
            <div className="photo-viewer-image">
              {selected !== undefined ? (
                <PhotoImage
                  key={selected}
                  src={photoUrl(photos.album.id, selected, "preview")}
                  alt={`Photo ${position} from ${photos.album.name}`}
                />
              ) : null}
            </div>
            <footer className="photo-viewer-footer">
              <Button
                variant="outline"
                size="key-icon"
                disabled={current === 0}
                title="Previous photo"
                onClick={() => go(-1)}
              >
                <ArrowLeftIcon size={20} aria-hidden="true" />
                <span className="sr-only">← Previous</span>
              </Button>
              <span className="sr-only" aria-live="polite">
                {position} / {photos.album.photo_count}
              </span>
              <Button
                variant="outline"
                size="key-icon"
                disabled={current === photos.assets.length - 1}
                title="Next photo"
                onClick={() => go(1)}
              >
                <ArrowRightIcon size={20} aria-hidden="true" />
                <span className="sr-only">Next →</span>
              </Button>
            </footer>
          </DialogContent>
        </Dialog>
      )}
      {photos.page === 0 && photos.next_page === null ? null : (
        <nav className="photo-pagination" aria-label="Photo pages">
          <Button
            variant="outline"
            size="key-icon"
            disabled={photos.page === 0}
            title="Previous page"
            onClick={() => select(photos.album.id, photos.page - 1)}
          >
            <ArrowLeftIcon size={20} aria-hidden="true" />
            <span className="sr-only">Previous page</span>
          </Button>
          <span className="num">{photos.page + 1}</span>
          <span className="sr-only">Page {photos.page + 1}</span>
          <Button
            variant="outline"
            size="key-icon"
            disabled={photos.next_page === null}
            title="Next page"
            onClick={() => {
              if (photos.next_page !== null)
                select(photos.album.id, photos.next_page)
            }}
          >
            <ArrowRightIcon size={20} aria-hidden="true" />
            <span className="sr-only">Next page</span>
          </Button>
        </nav>
      )}
    </>
  )
}

function PhotoImage({
  src,
  alt,
  lazy = false,
}: {
  src: string
  alt: string
  lazy?: boolean
}): React.JSX.Element {
  const [state, setState] = useState<"loading" | "loaded" | "failed">("loading")
  const [attempt, setAttempt] = useState(0)

  const imageRef = useCallback((image: HTMLImageElement | null) => {
    // SSR thumbnails can finish before React attaches its load/error listeners.
    if (image !== null && image.complete)
      setState(image.naturalWidth > 0 ? "loaded" : "failed")
  }, [])

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
            onLoad={() => setState("loaded")}
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
