import { useEffectEvent, useState } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { ImageBrokenIcon } from "@phosphor-icons/react/dist/csr/ImageBroken"
import { Button } from "@/components/ui/button"
import { photoUrl, type WallItem } from "@/lib/photos"
import { cn } from "@/lib/utils"

const DEFAULT_RATIO = 1.5

const ratios = new Map<string, number>()

function ratioOf(width: number, height: number): number {
  return Math.min(2.4, Math.max(0.5, width / height))
}

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
      data-slot="photo-cell"
      className="relative photo-cell min-w-0"
      // SAFETY: CSSProperties omits custom properties; --ratio is the photo's width over height.
      style={{ "--ratio": ratio } as React.CSSProperties}
    >
      <button
        ref={register}
        type="button"
        className="group/tile absolute inset-0 block overflow-hidden rounded-sm bg-wash-strong focus-visible:z-1 focus-visible:outline-3 focus-visible:outline-offset-2"
        aria-label={label}
        onClick={open}
      >
        <PhotoImage
          src={photoUrl(item.album, item.asset, "thumbnail")}
          alt={alt}
          variant="tile"
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
  variant,
  measure,
}: {
  src: string
  alt: string
  variant: "tile" | "viewer"
  measure?: (width: number, height: number) => void
}): React.JSX.Element {
  const [state, setState] = useState<"loading" | "loaded" | "failed">("loading")
  const [attempt, setAttempt] = useState(0)
  const tile = variant === "tile"

  const statusClass = cn(
    "absolute inset-0 flex flex-col items-center justify-center gap-4",
    tile ? "text-muted-ink" : "text-photo-ink/72"
  )

  const settle = useEffectEvent((image: HTMLImageElement) => {
    if (image.naturalWidth > 0) {
      setState("loaded")
      measure?.(image.naturalWidth, image.naturalHeight)
    } else setState("failed")
  })

  const imageRef = (image: HTMLImageElement | null): void => {
    if (image !== null && image.complete) settle(image)
  }

  return (
    <div
      className={cn(
        "group/image relative size-full",
        tile && "data-[state=loading]:bg-wash-strong"
      )}
      data-state={state}
    >
      {state === "failed" ? (
        <div className={statusClass} role="status">
          <ImageBrokenIcon
            size={tile ? 28 : 48}
            weight="light"
            aria-hidden="true"
          />
          <span className="sr-only">Photo unavailable</span>
          {tile ? null : (
            <Button
              variant="viewer"
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
            className={cn(
              "size-full group-data-[state=loading]/image:opacity-0",
              tile
                ? "object-cover transition-transform duration-slow ease-app group-hover/tile:scale-103 group-active/tile:scale-101"
                : "object-contain"
            )}
            loading={tile ? "lazy" : "eager"}
            decoding="async"
            onLoad={(event) => settle(event.currentTarget)}
            onError={() => setState("failed")}
          />
          {state === "loading" ? (
            <span className={statusClass}>
              <span className="sr-only">Loading photo…</span>
            </span>
          ) : null}
        </>
      )}
    </div>
  )
}
