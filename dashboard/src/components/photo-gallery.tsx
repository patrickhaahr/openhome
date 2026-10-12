import { useCallback, useState } from "react"
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
  select: (album: string, page: number) => void
  refresh: () => Promise<void>
}

/** Selected private albums, one bounded thumbnail page and a focus-managed full-screen viewer. */
export function PhotoGallery({
  albums,
  page,
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

  const retryButton = (
    <Button
      variant="outline"
      size="lg"
      disabled={retrying}
      onClick={() => void retry()}
    >
      {retrying ? "Refreshing…" : "Try again"}
    </Button>
  )

  if (albums.state !== "ready")
    return (
      <div className="photo-empty" role="status">
        <p className="page-eyebrow">Photos</p>
        <h2>
          {albums.reason === "missing-configuration"
            ? "Photo setup needed"
            : "Photo library unavailable"}
        </h2>
        <p>
          {albums.reason === "missing-configuration"
            ? "No photo library is connected here. Choose albums in the private OpenHome API configuration."
            : "Check Connection or try again. Your other dashboard features are still available."}
        </p>
        {retryButton}
        {retryFailed ? <p>Could not refresh. Try again.</p> : null}
      </div>
    )

  return (
    <>
      <div className="photo-toolbar">
        <nav className="photo-albums" aria-label="Photo albums">
          {albums.albums.map((album) => (
            <Button
              key={album.id}
              variant={
                page?.state === "ready" && page.photos.album.id === album.id
                  ? "default"
                  : "outline"
              }
              size="lg"
              disabled={album.missing}
              aria-pressed={
                page?.state === "ready" && page.photos.album.id === album.id
              }
              onClick={() => select(album.id, 0)}
            >
              <span className="photo-album-name">{album.name}</span>
              <span className="photo-album-count">
                {album.missing ? "Missing" : album.photo_count}
              </span>
            </Button>
          ))}
        </nav>
        <div className="photo-refresh">
          <Button
            variant="ghost"
            disabled={retrying}
            onClick={() => void retry()}
          >
            {retrying ? "Refreshing…" : "Refresh photos"}
          </Button>
        </div>
      </div>
      {page?.state === "ready" ? (
        <AlbumPhotos
          key={`${page.photos.album.id}/${page.photos.page}/${revision}`}
          photos={page.photos}
          select={select}
        />
      ) : (
        <div className="photo-empty" role="status">
          <h2>
            {page?.reason === "invalid-page"
              ? "This page is unavailable"
              : "Album unavailable"}
          </h2>
          <p>
            {page?.reason === "not-found"
              ? "This album was removed or is not selected for the dashboard. Choose an available album."
              : "Choose an available album or try again."}
          </p>
          {page?.reason === "invalid-page" ? (
            <Button
              size="lg"
              onClick={() => {
                const album = albums.albums.find((item) => !item.missing)

                if (album) select(album.id, 0)
              }}
            >
              First page
            </Button>
          ) : null}
          {retryButton}
        </div>
      )}
      {retryFailed ? <p role="status">Could not refresh. Try again.</p> : null}
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
      <div className="photo-section-heading">
        <div>
          <p className="page-eyebrow">Selected album</p>
          <h2>{photos.album.name}</h2>
        </div>
        <p>
          {photos.album.photo_count} photos
          <span> · Page {photos.page + 1}</span>
        </p>
      </div>
      {photos.assets.length === 0 ? (
        <div className="photo-empty">
          <h3>No photos here yet</h3>
          <p>This album has no available still photos.</p>
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
                <span className="photo-tile-number" aria-hidden="true">
                  {String(photos.page * 60 + index + 1).padStart(2, "0")}
                </span>
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
              <div>
                <DialogTitle>{photos.album.name}</DialogTitle>
                <DialogDescription>
                  Photo {position} of {photos.album.photo_count}
                </DialogDescription>
              </div>
              <DialogClose render={<Button variant="outline" size="lg" />}>
                Close photo
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
                size="lg"
                disabled={current === 0}
                onClick={() => go(-1)}
              >
                ← Previous
              </Button>
              <span aria-live="polite">
                {position} / {photos.album.photo_count}
              </span>
              <Button
                variant="outline"
                size="lg"
                disabled={current === photos.assets.length - 1}
                onClick={() => go(1)}
              >
                Next →
              </Button>
            </footer>
          </DialogContent>
        </Dialog>
      )}
      <nav className="photo-pagination" aria-label="Photo pages">
        <Button
          variant="outline"
          size="lg"
          disabled={photos.page === 0}
          onClick={() => select(photos.album.id, photos.page - 1)}
        >
          Previous page
        </Button>
        <span>Page {photos.page + 1}</span>
        <Button
          variant="outline"
          size="lg"
          disabled={photos.next_page === null}
          onClick={() => {
            if (photos.next_page !== null)
              select(photos.album.id, photos.next_page)
          }}
        >
          Next page
        </Button>
      </nav>
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
          <span>Photo unavailable</span>
          {lazy ? null : (
            <Button
              variant="outline"
              size="lg"
              onClick={() => {
                setAttempt((value) => value + 1)
                setState("loading")
              }}
            >
              Retry photo
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
            <span className="photo-image-status">Loading photo…</span>
          ) : null}
        </>
      )}
    </div>
  )
}
