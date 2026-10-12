import { WatchlistPoster } from "./watchlist-poster"
import { useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  type MediaMetadata,
  type MediaSearchResult,
  type WatchlistEntry,
  type WatchlistCommand,
} from "@/lib/watchlist"

/** One deliberate lookup through the private Axum provider adapter. */
export type SearchMedia = (input: {
  kind: WatchlistEntry["kind"]
  query: string
}) => Promise<MediaSearchResult>

/** Search and selection stay optional alongside every manual addition. */
export function WatchlistMediaSearch({
  query,
  kind,
  disabled,
  search,
  select,
}: {
  query: string
  kind: WatchlistEntry["kind"]
  disabled: boolean
  search: SearchMedia
  select: (result: MediaMetadata) => void
}): React.JSX.Element {
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)

  const [result, setResult] = useState<{
    query: string
    kind: WatchlistEntry["kind"]
    data: MediaSearchResult
  }>()

  const normalized = query.trim()

  const current =
    result?.query === normalized && result.kind === kind
      ? result.data
      : undefined

  async function lookup(): Promise<void> {
    if (locked.current) return
    locked.current = true
    setBusy(true)

    try {
      let data: MediaSearchResult

      try {
        data = await search({ kind, query: normalized })
      } catch {
        data = { state: "unavailable" }
      }

      setResult({ kind, query: normalized, data })
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  return (
    <div className="watchlist-media-search">
      <Button
        type="button"
        variant="outline"
        disabled={
          disabled || busy || normalized.length === 0 || normalized.length > 200
        }
        onClick={() => void lookup()}
      >
        {busy ? "Searching TMDB…" : "Search TMDB"}
      </Button>
      <p>Find a poster and description, or keep your own title.</p>
      <div role="status" aria-live="polite">
        {current?.state === "unavailable" ? (
          <p>TMDB is unavailable. You can still add a plan manually.</p>
        ) : null}
        {current?.state === "available" && current.results.length === 0 ? (
          <p>
            No titles found. Try another search or add your own title manually.
          </p>
        ) : null}
      </div>
      {current?.state === "available" && current.results.length > 0 ? (
        <ul className="watchlist-search-results" aria-label="TMDB titles">
          {current.results.map((item) => (
            <li key={`${item.kind}:${item.tmdb_id}`}>
              {item.poster_path !== null ? (
                <WatchlistPoster path={item.poster_path} />
              ) : (
                <div
                  className="watchlist-poster-placeholder"
                  aria-hidden="true"
                >
                  {item.kind === "movie" ? "Movie" : "Series"}
                </div>
              )}
              <div>
                <h4>{item.title}</h4>
                <p>
                  {item.kind === "movie" ? "Movie" : "Whole series"}
                  {item.release_date !== null
                    ? ` · ${item.release_date.slice(0, 4)}`
                    : ""}
                </p>
                {item.overview !== null ? (
                  <p className="watchlist-overview">{item.overview}</p>
                ) : null}
                <Button
                  type="button"
                  variant="secondary"
                  disabled={disabled}
                  onClick={() => select(item)}
                >
                  Select {item.title}
                </Button>
              </div>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  )
}

/** Existing plans can gain or refresh metadata without editing personal progress. */
export function WatchlistMetadataDetails({
  entry,
  ownerName,
  disabled,
  search,
  save,
}: {
  entry: WatchlistEntry
  ownerName: string
  disabled: boolean
  search: SearchMedia
  save: (command: WatchlistCommand) => Promise<boolean>
}): React.JSX.Element {
  const [query, setQuery] = useState(entry.title)

  return (
    <details className="watchlist-metadata-details">
      <summary>Find title details</summary>
      <p>
        Choose metadata for {ownerName}’s{" "}
        {entry.kind === "movie" ? "movie" : "whole series"}. Your status and
        dates stay as recorded.
      </p>
      <label>
        Search title for {entry.title} / {ownerName}
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          maxLength={200}
          disabled={disabled}
        />
      </label>
      <WatchlistMediaSearch
        query={query}
        kind={entry.kind}
        disabled={disabled}
        search={search}
        select={(item) => {
          void save({
            action: "enrich",
            owner_id: entry.owner_id,
            entry_id: entry.id,
            tmdb_id: item.tmdb_id,
          })
        }}
      />
    </details>
  )
}

/** TMDB's required notice and approved logo remain visible alongside metadata. */
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
