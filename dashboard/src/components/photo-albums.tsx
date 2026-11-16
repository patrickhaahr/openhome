import { useCallback, useState } from "react"
import { ImageBrokenIcon } from "@phosphor-icons/react/dist/csr/ImageBroken"
import { ImagesIcon } from "@phosphor-icons/react/dist/csr/Images"
import { StackIcon } from "@phosphor-icons/react/dist/csr/Stack"
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
