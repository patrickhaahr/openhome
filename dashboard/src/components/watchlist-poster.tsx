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
    <span className="kind-glyph">
      {kind === "movie" ? (
        <FilmStripIcon size={size} aria-hidden="true" />
      ) : (
        <TelevisionSimpleIcon size={size} aria-hidden="true" />
      )}
      <span className="sr-only">{kind === "movie" ? "Movie" : "Series"}</span>
    </span>
  )
}

/**
 * TMDB artwork over a typographic poster: without artwork, or when it fails to load, the title
 * set large is the poster.
 */
export function WatchlistPoster({
  title,
  path,
}: {
  title: string
  path: string | null
}): React.JSX.Element {
  return (
    <span className="poster">
      <span className="poster-type" aria-hidden="true">
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
          referrerPolicy="no-referrer"
          onError={(event) => {
            event.currentTarget.hidden = true
          }}
          onLoad={(event) => {
            event.currentTarget.hidden = false
          }}
        />
      )}
    </span>
  )
}

/** TMDB's required notice and approved logo, kept beside its metadata. */
export function TmdbCredits(): React.JSX.Element {
  return (
    <aside className="tmdb-credits" aria-label="TMDB credits">
      <a href="https://www.themoviedb.org" target="_blank" rel="noreferrer">
        <img src="/tmdb-logo.svg" alt="TMDB" width={100} height={13} />
      </a>
      <p>
        This product uses the TMDB API but is not endorsed or certified by TMDB.
      </p>
    </aside>
  )
}
