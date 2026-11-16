import { cva, type VariantProps } from "class-variance-authority"
import { FilmStripIcon } from "@phosphor-icons/react/dist/csr/FilmStrip"
import { TelevisionSimpleIcon } from "@phosphor-icons/react/dist/csr/TelevisionSimple"
import { posterUrl, type WatchlistEntry } from "@/lib/watchlist"

/** Movie or series as a quiet glyph; the word stays for assistive technology. */
export function KindGlyph({
  kind,
  size = 16,
}: {
  kind: WatchlistEntry["kind"]
  size?: number
}): React.JSX.Element {
  return (
    <span className="inline-flex">
      {kind === "movie" ? (
        <FilmStripIcon size={size} aria-hidden="true" />
      ) : (
        <TelevisionSimpleIcon size={size} aria-hidden="true" />
      )}
      <span className="sr-only">{kind === "movie" ? "Movie" : "Series"}</span>
    </span>
  )
}

const posterType = cva(
  "absolute inset-0 flex overflow-hidden p-3 leading-poster font-semibold tracking-headline text-balance text-ink",
  {
    variants: {
      size: { plate: "text-poster", result: "text-base" },
      chip: { true: "items-start", false: "items-end" },
    },
    defaultVariants: { size: "plate", chip: false },
  }
)

/**
 * TMDB artwork over a typographic poster: without artwork, or when it fails to load, the title
 * set large is the poster. A `chip` sits over its bottom-left corner and moves the title up.
 */
export function WatchlistPoster({
  title,
  path,
  size,
  chip,
}: {
  title: string
  path: string | null
  chip?: React.ReactNode
} & Pick<VariantProps<typeof posterType>, "size">): React.JSX.Element {
  return (
    <span className="relative block aspect-2/3 overflow-hidden rounded-poster bg-wash-strong">
      <span
        className={posterType({ size, chip: chip !== undefined })}
        aria-hidden="true"
      >
        {title}
      </span>
      {path === null ? null : (
        <img
          key={path}
          src={posterUrl(path)}
          alt=""
          width={342}
          height={513}
          loading="lazy"
          decoding="async"
          className="absolute inset-0 size-full object-cover transition-transform duration-mid ease-app group-hover/title:scale-103"
          referrerPolicy="no-referrer"
          onError={(event) => {
            event.currentTarget.hidden = true
          }}
          onLoad={(event) => {
            event.currentTarget.hidden = false
          }}
        />
      )}
      {chip === undefined ? null : (
        <span className="absolute bottom-2 left-2">{chip}</span>
      )}
    </span>
  )
}

/** TMDB's required notice and approved logo, kept beside its metadata. */
export function TmdbCredits(): React.JSX.Element {
  return (
    <aside
      className="flex flex-wrap items-center gap-x-4 gap-y-2 text-label text-muted-ink"
      aria-label="TMDB credits"
    >
      <a
        className="grid min-h-11 items-center"
        href="https://www.themoviedb.org"
        target="_blank"
        rel="noreferrer"
      >
        <img src="/tmdb-logo.svg" alt="TMDB" width={100} height={13} />
      </a>
      <p>
        This product uses the TMDB API but is not endorsed or certified by TMDB.
      </p>
    </aside>
  )
}
