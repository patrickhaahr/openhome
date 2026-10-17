import { useId, useState } from "react"
import { BookmarkSimpleIcon } from "@phosphor-icons/react/dist/csr/BookmarkSimple"
import { CalendarBlankIcon } from "@phosphor-icons/react/dist/csr/CalendarBlank"
import { CheckCircleIcon } from "@phosphor-icons/react/dist/csr/CheckCircle"
import { MagnifyingGlassIcon } from "@phosphor-icons/react/dist/csr/MagnifyingGlass"
import { PlayIcon } from "@phosphor-icons/react/dist/csr/Play"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { PersonDisc, Segments } from "@/components/list-parts"
import {
  MediaResults,
  useMediaSearch,
  type SearchMedia,
} from "@/components/watch-add"
import {
  KindGlyph,
  TmdbCredits,
  WatchlistPoster,
} from "@/components/watchlist-poster"
import { formatDay, trimText } from "@/lib/lists"
import type { WatchlistCommand, WatchlistEntry } from "@/lib/watchlist"
import type { HouseholdUser } from "@/lib/wishlist"

const statuses = [
  {
    value: "planned",
    label: "Planned",
    icon: <BookmarkSimpleIcon size={18} aria-hidden="true" />,
  },
  {
    value: "watching",
    label: "Watching",
    icon: <PlayIcon size={18} aria-hidden="true" />,
  },
  {
    value: "completed",
    label: "Completed",
    icon: <CheckCircleIcon size={18} aria-hidden="true" />,
  },
] as const satisfies ReadonlyArray<{
  value: WatchlistEntry["status"]
  label: string
  icon: React.ReactNode
}>

/**
 * One title: poster, story, whose it is and when; the status is three keys that save at once
 * (Axum records today on completion). Dates, details and delete follow.
 */
export function WatchDetail({
  entry,
  owner,
  busy,
  search,
  save,
  done,
}: {
  entry: WatchlistEntry
  owner: HouseholdUser
  busy: boolean
  search: SearchMedia
  save: (command: WatchlistCommand, saved: string) => Promise<boolean>
  done: () => void
}): React.JSX.Element {
  const id = useId()
  const [mode, setMode] = useState<"view" | "dates" | "find" | "delete">("view")

  const [added, setAdded] = useState(entry.date_added)
  const [completed, setCompleted] = useState(entry.completion_date ?? "")
  const [query, setQuery] = useState(entry.title)
  const finder = useMediaSearch(search, [entry.kind])
  const target = { owner_id: owner.id, entry_id: entry.id }

  return (
    <div className="sheet-body">
      <div className="watch-detail-head">
        <WatchlistPoster title={entry.title} path={entry.poster_path} />
        <div className="watch-detail-text">
          <p className="sheet-owner">
            <PersonDisc name={owner.name} />
            {owner.name}
          </p>
          {/* The sheet's title already names the entry for assistive technology. */}
          <p className="sheet-name" aria-hidden="true">
            {entry.title}
          </p>
          <p className="media-result-meta">
            <KindGlyph kind={entry.kind} />
            {entry.release_date === null ? null : (
              <span className="num">{entry.release_date.slice(0, 4)}</span>
            )}
          </p>
          {entry.overview === null ? null : (
            <p className="sheet-notes">{entry.overview}</p>
          )}
        </div>
      </div>
      <Segments
        label="Status"
        value={entry.status}
        options={statuses}
        disabled={busy}
        choose={(status) => {
          if (status !== entry.status)
            void save(
              { action: "edit", ...target, status },
              `${entry.title} is now ${status}.`
            )
        }}
      />
      {mode === "dates" ? (
        <form
          className="watch-dates"
          aria-label="Change dates"
          onSubmit={(event) => {
            event.preventDefault()

            void save(
              {
                action: "edit",
                ...target,
                date_added: added === entry.date_added ? undefined : added,
                completion_date:
                  entry.status === "completed" &&
                  completed !== "" &&
                  completed !== entry.completion_date
                    ? completed
                    : undefined,
              },
              "Dates saved."
            ).then((saved) => {
              if (saved) setMode("view")
            })
          }}
        >
          <label className="field" htmlFor={`${id}-added`}>
            <span>Added</span>
            <Input
              id={`${id}-added`}
              type="date"
              value={added}
              required
              disabled={busy}
              onChange={(event) => setAdded(event.target.value)}
            />
          </label>
          {entry.status === "completed" ? (
            <label className="field" htmlFor={`${id}-completed`}>
              <span>Completed</span>
              <Input
                id={`${id}-completed`}
                type="date"
                min={added}
                value={completed}
                disabled={busy}
                onChange={(event) => setCompleted(event.target.value)}
              />
            </label>
          ) : null}
          <div className="sheet-actions">
            <Button type="submit" size="key" disabled={busy}>
              Save dates
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="key"
              disabled={busy}
              onClick={() => setMode("view")}
            >
              Cancel
            </Button>
          </div>
        </form>
      ) : (
        <dl className="watch-days">
          <div>
            <dt>Added</dt>
            <dd className="num">
              <time dateTime={entry.date_added}>
                {formatDay(entry.date_added)}
              </time>
            </dd>
          </div>
          {entry.completion_date === null ? null : (
            <div>
              <dt>Completed</dt>
              <dd className="num">
                <time dateTime={entry.completion_date}>
                  {formatDay(entry.completion_date)}
                </time>
              </dd>
            </div>
          )}
          <Button
            variant="outline"
            size="key-icon"
            title="Change dates"
            disabled={busy}
            onClick={() => {
              setAdded(entry.date_added)
              setCompleted(entry.completion_date ?? "")
              setMode("dates")
            }}
          >
            <CalendarBlankIcon size={20} aria-hidden="true" />
            <span className="sr-only">Change dates</span>
          </Button>
        </dl>
      )}
      {mode === "find" ? (
        <div className="watch-find">
          <label className="field" htmlFor={`${id}-find`}>
            <span>Find details on TMDB</span>
            <span className="link-input">
              <MagnifyingGlassIcon size={22} aria-hidden="true" />
              <Input
                id={`${id}-find`}
                type="search"
                value={query}
                autoFocus
                maxLength={200}
                onChange={(event) => {
                  setQuery(event.target.value)
                  finder.type(event.target.value)
                }}
              />
            </span>
          </label>
          <MediaResults
            found={finder.found}
            disabled={busy}
            pick={(result) => {
              void save(
                { action: "enrich", ...target, tmdb_id: result.tmdb_id },
                "Details added. Status and dates are unchanged."
              ).then((saved) => {
                if (saved) setMode("view")
              })
            }}
          />
        </div>
      ) : null}
      {mode === "delete" ? (
        <div className="sheet-confirm" role="group" aria-label="Delete entry">
          <p>
            Delete “{entry.title}” from {owner.name}’s watchlist? This cannot be
            undone.
          </p>
          <div className="sheet-actions">
            <Button
              variant="destructive"
              size="key"
              disabled={busy}
              onClick={() => {
                void save(
                  { action: "remove", ...target },
                  `${entry.title} deleted.`
                ).then((saved) => {
                  if (saved) done()
                })
              }}
            >
              <TrashIcon size={18} aria-hidden="true" />
              Delete for good
            </Button>
            <Button
              variant="ghost"
              size="key"
              disabled={busy}
              onClick={() => setMode("view")}
            >
              Keep it
            </Button>
          </div>
        </div>
      ) : (
        <div className="sheet-actions sheet-actions-end">
          {mode === "find" ? null : (
            <Button
              variant="outline"
              size="key"
              disabled={busy}
              onClick={() => {
                setQuery(entry.title)
                finder.type(trimText(entry.title))
                setMode("find")
              }}
            >
              <MagnifyingGlassIcon size={18} aria-hidden="true" />
              {entry.tmdb_id === null ? "Find details" : "Change details"}
            </Button>
          )}
          <Button
            variant="destructive"
            size="key"
            disabled={busy}
            onClick={() => setMode("delete")}
          >
            <TrashIcon size={18} aria-hidden="true" />
            Delete
          </Button>
        </div>
      )}
      {entry.tmdb_id === null && mode !== "find" ? null : <TmdbCredits />}
    </div>
  )
}
