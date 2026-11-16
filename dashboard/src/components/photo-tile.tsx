import { useEffectEvent, useState } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { ImageBrokenIcon } from "@phosphor-icons/react/dist/csr/ImageBroken"
import { Button } from "@/components/ui/button"
import { photoUrl, type WallItem } from "@/lib/photos"

// Thumbnails tell their own frame once loaded; until then a tile is a 3:2 landscape.
const DEFAULT_RATIO = 1.5

const ratios = new Map<string, number>()

function ratioOf(width: number, height: number): number {
  return Math.min(2.4, Math.max(0.5, width / height))
}

/**
 * The wall's single page request, owned by `load` in `PhotoWall`: idle, reading the page at a
 * cursor, or that read failed and the same cursor is offered again. Loaded items and the next
 * cursor are separate view data that a failure never touches.
 */

export function Tile({
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

export function PhotoImage({
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
