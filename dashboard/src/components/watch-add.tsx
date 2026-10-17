import { useEffect, useId, useRef, useState } from "react"
import { ArrowLeftIcon } from "@phosphor-icons/react/dist/csr/ArrowLeft"
import { CircleNotchIcon } from "@phosphor-icons/react/dist/csr/CircleNotch"
import { MagnifyingGlassIcon } from "@phosphor-icons/react/dist/csr/MagnifyingGlass"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { PersonDisc } from "@/components/list-parts"
import { KindGlyph, WatchlistPoster } from "@/components/watchlist-poster"
import { trimText } from "@/lib/lists"
import { createRequestId } from "@/lib/request-id"
import {
  watchlistCommand,
  type MediaMetadata,
  type MediaSearchResult,
  type WatchlistCommand,
  type WatchlistEntry,
} from "@/lib/watchlist"
import type { HouseholdUser } from "@/lib/wishlist"

/** One deliberate lookup through the private Axum provider adapter. */
export type SearchMedia = (input: {
  kind: WatchlistEntry["kind"]
  query: string
}) => Promise<MediaSearchResult>

type Found =
  | { state: "idle" }
  | { state: "searching"; query: string }
  | { state: "found"; query: string; results: MediaMetadata[] }
  | { state: "unavailable"; query: string }

/** The search state and the keystroke that feeds it. */
export type MediaSearch = { found: Found; type: (query: string) => void }

/**
 * Search as you type, gently: 400 ms after the last key, at least two characters, both kinds at
 * once; a newer query makes older answers stale and they are dropped.
 */
export function useMediaSearch(
  search: SearchMedia,
  kinds: ReadonlyArray<WatchlistEntry["kind"]> = ["movie", "series"]
): MediaSearch {
  const [found, setFound] = useState<Found>({ state: "idle" })
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const latest = useRef(0)

  // A closed plate searches nothing more.
  useEffect(() => () => clearTimeout(timer.current), [])

  function type(text: string): void {
    clearTimeout(timer.current)
    const query = trimText(text)
    const ticket = ++latest.current

    if (Array.from(query).length < 2 || query.length > 200) {
      setFound({ state: "idle" })

      return
    }

    timer.current = setTimeout(() => {
      setFound({ state: "searching", query })

      void Promise.all(
        kinds.map((kind) =>
          search({ kind, query }).catch((): MediaSearchResult => ({
            state: "unavailable",
          }))
        )
      ).then((answers) => {
        if (ticket !== latest.current) return

        if (answers.every((answer) => answer.state === "unavailable")) {
          setFound({ state: "unavailable", query })

          return
        }

        // Interleave movies and series so the best of each comes first.
        const lists = answers.map((answer) =>
          answer.state === "available" ? answer.results : []
        )

        const results: MediaMetadata[] = []
        const longest = Math.max(...lists.map((list) => list.length))

        for (let index = 0; index < longest; index++)
          for (const list of lists) {
            const result = list[index]

            if (result !== undefined) results.push(result)
          }

        setFound({ state: "found", query, results: results.slice(0, 12) })
      })
    }, 400)
  }

  return { found, type }
}

/** Search results as small posters; each one is a key. */
export function MediaResults({
  found,
  pick,
  disabled,
}: {
  found: Found
  pick: (result: MediaMetadata) => void
  disabled: boolean
}): React.JSX.Element {
  return (
    <>
      <p className="link-status" role="status">
        {found.state === "searching"
          ? "Searching TMDB…"
          : found.state === "unavailable"
            ? "TMDB is not answering. You can still add the title by hand."
            : found.state === "found" && found.results.length === 0
              ? "Nothing found on TMDB. Add it by hand below."
              : found.state === "found"
                ? `${found.results.length} ${found.results.length === 1 ? "title" : "titles"} found`
                : ""}
      </p>
      {found.state === "found" && found.results.length > 0 ? (
        <ul className="media-results" aria-label="TMDB titles">
          {found.results.map((result) => (
            <li key={`${result.kind}:${result.tmdb_id}`}>
              <button
                type="button"
                className="media-result"
                disabled={disabled}
                onClick={() => pick(result)}
              >
                <WatchlistPoster
                  title={result.title}
                  path={result.poster_path}
                />
                <span className="media-result-text">
                  <span className="media-result-title">{result.title}</span>
                  <span className="media-result-meta">
                    <KindGlyph kind={result.kind} size={14} />
                    {result.release_date === null ? null : (
                      <span className="num">
                        {result.release_date.slice(0, 4)}
                      </span>
                    )}
                  </span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </>
  )
}

type Plan = Extract<WatchlistCommand, { action: "add" }>

type Choice = Pick<
  MediaMetadata,
  "kind" | "title" | "poster_path" | "release_date"
> & { tmdb_id: number | undefined }

function readPending(ownerId: string): Plan | undefined {
  try {
    const parsed = watchlistCommand.safeParse(
      JSON.parse(
        sessionStorage.getItem(`openhome.pending-watch.${ownerId}`) ?? "null"
      )
    )

    if (parsed.success && parsed.data.action === "add") return parsed.data
  } catch {
    /* Private browsing may disable storage. */
  }

  return undefined
}

function storePending(ownerId: string, plan: Plan | undefined): void {
  try {
    const key = `openhome.pending-watch.${ownerId}`

    if (plan) sessionStorage.setItem(key, JSON.stringify(plan))
    else sessionStorage.removeItem(key)
  } catch {
    /* The mounted flow keeps its in-memory retry identity. */
  }
}

/** Find the title, then say who it is for; one tap on a person adds it. */
export function WatchAdd({
  owners,
  person,
  busy,
  search,
  save,
  done,
}: {
  owners: readonly HouseholdUser[]
  person: string | undefined
  busy: boolean
  search: SearchMedia
  save: (command: WatchlistCommand, saved: string) => Promise<boolean>
  done: () => void
}): React.JSX.Element {
  const id = useId()
  const [query, setQuery] = useState("")
  const [choice, setChoice] = useState<Choice>()
  const { found, type } = useMediaSearch(search)
  const pending = useRef<Plan | undefined>(undefined)

  async function add(owner: HouseholdUser, chosen: Choice): Promise<void> {
    const title = trimText(chosen.title)

    // A lost response retried with the same title and person keeps its request ID.
    const previous = pending.current ?? readPending(owner.id)

    pending.current =
      previous !== undefined &&
      previous.owner_id === owner.id &&
      previous.title === title &&
      previous.kind === chosen.kind &&
      previous.tmdb_id === chosen.tmdb_id
        ? previous
        : {
            action: "add",
            owner_id: owner.id,
            kind: chosen.kind,
            title,
            request_id: createRequestId(),
            tmdb_id: chosen.tmdb_id,
          }

    storePending(owner.id, pending.current)

    if (
      await save(pending.current, `${title} is on ${owner.name}’s watchlist.`)
    ) {
      pending.current = undefined
      storePending(owner.id, undefined)
      done()
    }
  }

  if (choice !== undefined)
    return (
      <div className="sheet-body">
        <div className="watch-choice">
          <WatchlistPoster title={choice.title} path={choice.poster_path} />
          <div>
            <p className="sheet-name">{choice.title}</p>
            <p className="media-result-meta">
              <KindGlyph kind={choice.kind} />
              {choice.release_date === null ? null : (
                <span className="num">{choice.release_date.slice(0, 4)}</span>
              )}
            </p>
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => setChoice(undefined)}
            >
              <ArrowLeftIcon size={18} aria-hidden="true" />
              Another title
            </Button>
          </div>
        </div>
        <fieldset className="field destination">
          <legend>For whom?</legend>
          <div className="whom-keys">
            {owners.map((owner) => (
              <button
                key={owner.id}
                type="button"
                className="whom-key"
                data-suggested={owner.id === person}
                disabled={busy}
                onClick={() => void add(owner, choice)}
              >
                <PersonDisc name={owner.name} />
                {owner.name}
              </button>
            ))}
          </div>
        </fieldset>
      </div>
    )

  const typed = trimText(query)

  return (
    <div className="sheet-body">
      <label className="field" htmlFor={`${id}-find`}>
        <span>Find a movie or series</span>
        <span className="link-input">
          <MagnifyingGlassIcon size={22} aria-hidden="true" />
          <Input
            id={`${id}-find`}
            type="search"
            value={query}
            autoFocus
            autoComplete="off"
            maxLength={500}
            placeholder="Title"
            onChange={(event) => {
              setQuery(event.target.value)
              type(event.target.value)
            }}
          />
          {found.state === "searching" ? (
            <CircleNotchIcon size={22} className="spin" aria-hidden="true" />
          ) : null}
        </span>
      </label>
      <MediaResults
        found={found}
        disabled={busy}
        pick={(result) => setChoice(result)}
      />
      {typed === "" ? null : (
        <div className="manual-add" role="group" aria-label="Add by hand">
          <span>Not here? Add “{typed}” as a</span>
          {(["movie", "series"] as const).map((kind) => (
            <Button
              key={kind}
              variant="outline"
              disabled={busy}
              onClick={() =>
                setChoice({
                  kind,
                  title: typed,
                  poster_path: null,
                  release_date: null,
                  tmdb_id: undefined,
                })
              }
            >
              {kind === "movie" ? "Movie" : "Series"}
            </Button>
          ))}
        </div>
      )}
    </div>
  )
}
