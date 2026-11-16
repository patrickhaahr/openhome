import { useCallback, useState } from "react"
import { ImageBrokenIcon } from "@phosphor-icons/react/dist/csr/ImageBroken"
import { ImagesIcon } from "@phosphor-icons/react/dist/csr/Images"
import { StackIcon } from "@phosphor-icons/react/dist/csr/Stack"
import { cn } from "@/lib/utils"
import { formatWhole } from "@/lib/number-format"
import {
  ALL_PHOTOS,
  ON_THIS_DAY,
  formatMonthDay,
  librarySpan,
  monthSpan,
  photoUrl,
  type Album,
  type OnThisDayResult,
  type WallItem,
} from "@/lib/photos"

/** The count glyph and figure, and the months, under an album's name. */
export function AlbumMeta({
  count,
  span,
  unit,
  className,
}: {
  count: number
  span: string | null
  unit: "photos" | "albums"
  className?: string
}): React.JSX.Element {
  const Icon = unit === "photos" ? ImagesIcon : StackIcon

  return (
    <span
      className={cn(
        "mx-1.5 flex flex-wrap items-center gap-x-3.5 gap-y-1 text-control text-muted-ink max-phone:gap-x-2.5 max-phone:gap-y-0.5 max-phone:text-label",
        className
      )}
    >
      <span className="inline-flex items-center gap-1.5">
        <Icon size={18} aria-hidden="true" />
        <span className="num">{formatWhole(count)}</span>
        <span className="sr-only"> {unit}</span>
      </span>
      {span === null ? null : <span className="num">{span}</span>}
    </span>
  )
}

type Plate = "all" | "on-this-day" | "album"

const blank = "grid place-items-center text-muted-ink"

/** The cover mosaic: two or four photos in two columns, three as one tall photo beside two. */
function mosaic(plate: Plate, count: number): string {
  if (plate === "album") return ""

  if (count === 2 || count === 4) return "grid-cols-2"

  return count === 3 ? "grid-cols-mosaic" : ""
}

function AlbumPlate({
  plate,
  name,
  covers = 1,
  disabled = false,
  onClick,
  children,
  meta,
}: {
  plate: Plate
  name: string
  /** How many covers the plate shows, which decides its mosaic. */
  covers?: number
  disabled?: boolean
  onClick: () => void
  children: React.ReactNode
  meta: React.ReactNode
}): React.JSX.Element {
  const all = plate === "all"

  return (
    // All photos leads at two columns and two rows; its cover is a mosaic of the albums' covers.
    <li
      className={all ? "col-span-2 row-span-2 max-phone:row-auto" : undefined}
    >
      <button
        type="button"
        disabled={disabled}
        onClick={onClick}
        className="group/plate flex size-full flex-col rounded-lg bg-wash p-2 pb-3.5 text-start text-ink transition-plate duration-fast ease-app hover:bg-wash-strong active:scale-98 disabled:text-muted-ink max-phone:pb-3"
      >
        <span
          className={cn(
            "grid auto-rows-fr gap-0.75 overflow-hidden rounded-sm bg-wash-strong",
            all
              ? "min-h-48 grow basis-0 contain-size max-phone:aspect-video max-phone:min-h-0 max-phone:flex-none max-phone:contain-none"
              : "aspect-4/3",
            mosaic(plate, covers)
          )}
        >
          {children}
        </span>
        <span
          className={cn(
            "mx-1.5 mt-3.5 mb-1.5 line-clamp-2 font-medium wrap-anywhere max-phone:mt-2.5",
            all
              ? "text-title leading-poster tracking-tighter"
              : "text-value leading-photo-name tracking-title max-phone:text-body-lg"
          )}
        >
          {name}
        </span>
        {meta}
      </button>
    </li>
  )
}

export function AlbumIndex({
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
      <ul className="grid grid-flow-dense grid-cols-albums gap-plates max-phone:grid-cols-2 max-phone:gap-2.5">
        {available.length > 1 ? (
          <AlbumPlate
            plate="all"
            name="All photos"
            covers={Math.min(covers.length, 4)}
            onClick={() => open(ALL_PHOTOS)}
            meta={
              <AlbumMeta
                count={available.length}
                span={librarySpan(available)}
                unit="albums"
              />
            }
          >
            {covers.slice(0, 4).map((cover, at) => (
              <Cover
                key={cover.album}
                {...cover}
                tall={covers.length === 3 && at === 0}
              />
            ))}
          </AlbumPlate>
        ) : null}
        {today === null ? null : (
          <AlbumPlate
            plate="on-this-day"
            name="On this day"
            covers={Math.min(today.items.length, 4)}
            onClick={() => open(ON_THIS_DAY)}
            meta={
              <AlbumMeta
                count={today.total}
                span={formatMonthDay(today.date)}
                unit="photos"
              />
            }
          >
            {today.items.slice(0, 4).map((cover, at) => (
              <Cover
                key={cover.asset}
                album={cover.album}
                asset={cover.asset}
                tall={today.items.length === 3 && at === 0}
              />
            ))}
          </AlbumPlate>
        )}
        {albums.map((album) => (
          <AlbumPlate
            key={album.id}
            plate="album"
            name={album.name}
            disabled={album.missing}
            onClick={() => open(album.id)}
            meta={
              album.missing ? (
                <span className="sr-only">
                  Removed from the photo library or no longer shared
                </span>
              ) : (
                <AlbumMeta
                  count={album.photo_count}
                  span={monthSpan(album.first_month, album.last_month)}
                  unit="photos"
                />
              )
            }
          >
            {album.cover_id === null ? (
              <span className={blank}>
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
          </AlbumPlate>
        ))}
      </ul>
    </>
  )
}

/** A decorative cover: the album's name already names the plate. */
function Cover({
  album,
  asset,
  tall = false,
}: WallItem & { tall?: boolean }): React.JSX.Element {
  const [failed, setFailed] = useState(false)

  const imageRef = useCallback((image: HTMLImageElement | null) => {
    if (image?.complete === true && image.naturalWidth === 0) setFailed(true)
  }, [])

  return failed ? (
    <span className={blank}>
      <ImageBrokenIcon size={32} weight="light" aria-hidden="true" />
    </span>
  ) : (
    <img
      ref={imageRef}
      src={photoUrl(album, asset, "thumbnail")}
      alt=""
      className={cn(
        "size-full min-h-0 object-cover transition-transform duration-slow ease-app group-hover/plate:scale-103",
        tall && "row-span-2"
      )}
      loading="lazy"
      decoding="async"
      onError={() => setFailed(true)}
    />
  )
}
