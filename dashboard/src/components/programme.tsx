import { useState } from "react"
import { FilmSlateIcon } from "@phosphor-icons/react/dist/csr/FilmSlate"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Link } from "@tanstack/react-router"
import { OpenConnection } from "@/components/control-alert"
import {
  FeedbackLine,
  ListSheet,
  PersonDisc,
  PersonKeys,
  useListSave,
} from "@/components/list-parts"
import { Button } from "@/components/ui/button"
import { WatchAdd, type SearchMedia } from "@/components/watch-add"
import { WatchDetail } from "@/components/watch-sheet"
import {
  KindGlyph,
  TmdbCredits,
  WatchlistPoster,
} from "@/components/watchlist-poster"
import { formatDay, type PersonSearch } from "@/lib/lists"
import {
  watchlistFailureMessage,
  type WatchlistCommand,
  type WatchlistEntry,
  type WatchlistMutation,
  type WatchlistState,
} from "@/lib/watchlist"
import type { HouseholdUser } from "@/lib/wishlist"

type Open = { kind: "entry"; id: string } | { kind: "add" }

const rows = [
  { status: "watching", label: "Watching now" },
  { status: "planned", label: "Planned" },
  { status: "completed", label: "Completed" },
] as const satisfies ReadonlyArray<{
  status: WatchlistEntry["status"]
  label: string
}>

/**
 * The cinema programme: what is on now, what is planned and what has been seen, posters first.
 * The person keys narrow it to one Household User; tapping a poster opens its detail plate.
 */
export function Programme({
  data,
  search,
  choose,
  change,
  find,
  refresh,
  lead,
}: {
  data: WatchlistState
  search: PersonSearch
  choose: (search: PersonSearch) => void
  change: (command: WatchlistCommand) => Promise<WatchlistMutation>
  find: SearchMedia
  refresh: () => Promise<void>
  lead?: React.ReactNode
}): React.JSX.Element {
  const { busy, feedback, save } = useListSave(
    change,
    refresh,
    watchlistFailureMessage,
    "unreachable"
  )

  const [open, setOpen] = useState<Open>()

  if (data.state === "unavailable")
    return (
      <div className="lists">
        {lead === undefined ? null : <div className="lists-bar">{lead}</div>}
        <div className="lists-empty">
          <FilmSlateIcon size={48} weight="light" aria-hidden="true" />
          <h2>Watchlists unavailable</h2>
          <p>{watchlistFailureMessage[data.reason]}</p>
          <OpenConnection />
        </div>
      </div>
    )

  const { owners, entries } = data
  const owner = owners.find((candidate) => candidate.id === search.person)

  const ownerOf = (entry: WatchlistEntry): HouseholdUser | undefined =>
    owners.find((candidate) => candidate.id === entry.owner_id)

  const shown = entries.filter(
    (entry) => owner === undefined || entry.owner_id === owner.id
  )

  const counts = new Map<string, number>()

  for (const entry of entries)
    if (entry.status !== "completed")
      counts.set(entry.owner_id, (counts.get(entry.owner_id) ?? 0) + 1)

  const selected =
    open?.kind === "entry"
      ? entries.find((entry) => entry.id === open.id)
      : undefined

  const selectedOwner = selected === undefined ? undefined : ownerOf(selected)

  return (
    <div className="lists" aria-busy={busy}>
      <div className="lists-bar">
        {lead}
        {owners.length === 0 ? null : (
          <PersonKeys
            owners={owners}
            selected={owner?.id}
            counts={counts}
            noun={["title to watch", "titles to watch"]}
            choose={(person) => choose({ person })}
          />
        )}
        <div className="lists-bar-end">
          <Button
            size="key"
            disabled={owners.length === 0}
            onClick={() => setOpen({ kind: "add" })}
          >
            <PlusIcon size={20} weight="bold" aria-hidden="true" />
            Add a title
          </Button>
        </div>
      </div>
      {open === undefined ? (
        <FeedbackLine feedback={feedback} busy={busy} />
      ) : null}
      {owners.length === 0 ? (
        <div className="lists-empty">
          <FilmSlateIcon size={48} weight="light" aria-hidden="true" />
          <h2>No one here yet</h2>
          <p>Household members are added under Wish.</p>
          <Link to="/lists" search={{ tab: "wish" }} className="key-link-text">
            Open wishes
          </Link>
        </div>
      ) : (
        <div className="programme">
          {rows.map((row) => {
            const items = shown.filter((entry) => entry.status === row.status)

            if (row.status === "completed")
              items.sort((a, b) =>
                (b.completion_date ?? "").localeCompare(a.completion_date ?? "")
              )

            return (
              <section
                key={row.status}
                className="programme-row"
                data-status={row.status}
                aria-label={row.label}
              >
                <h2 className="board-heading">
                  {row.label}
                  <span className="board-count num">
                    {items.length}
                    <span className="sr-only">
                      {" "}
                      {items.length === 1 ? "title" : "titles"}
                    </span>
                  </span>
                </h2>
                {items.length === 0 ? (
                  <p className="programme-empty">
                    {row.status === "watching"
                      ? "Nothing on right now."
                      : row.status === "planned"
                        ? "Nothing planned yet."
                        : "Nothing finished yet."}
                  </p>
                ) : (
                  <ul className="programme-titles">
                    {items.map((entry) => (
                      <li key={entry.id}>
                        <ProgrammeTitle
                          entry={entry}
                          owner={
                            owner === undefined ? ownerOf(entry) : undefined
                          }
                          open={() => setOpen({ kind: "entry", id: entry.id })}
                        />
                      </li>
                    ))}
                  </ul>
                )}
              </section>
            )
          })}
          <TmdbCredits />
        </div>
      )}
      <ListSheet
        open={selected !== undefined}
        close={() => setOpen(undefined)}
        title={selected?.title ?? "Title"}
        titleHidden
      >
        {selected !== undefined && selectedOwner !== undefined ? (
          <>
            <WatchDetail
              key={selected.id}
              entry={selected}
              owner={selectedOwner}
              busy={busy}
              search={find}
              save={save}
              done={() => setOpen(undefined)}
            />
            <FeedbackLine feedback={feedback} busy={busy} />
          </>
        ) : null}
      </ListSheet>
      <ListSheet
        open={open?.kind === "add"}
        close={() => setOpen(undefined)}
        title="Add a title"
      >
        {open?.kind === "add" ? (
          <>
            <WatchAdd
              owners={owners}
              person={owner?.id}
              busy={busy}
              search={find}
              save={save}
              done={() => setOpen(undefined)}
            />
            <FeedbackLine feedback={feedback} busy={busy} />
          </>
        ) : null}
      </ListSheet>
    </div>
  )
}

/**
 * A poster and its line: on "Watching now" a wide plate with the story, elsewhere the poster
 * with its title beneath; completed titles carry their completion date.
 */
function ProgrammeTitle({
  entry,
  owner,
  open,
}: {
  entry: WatchlistEntry
  owner: HouseholdUser | undefined
  open: () => void
}): React.JSX.Element {
  return (
    <button type="button" className="programme-title" onClick={open}>
      <span className="programme-poster">
        <WatchlistPoster title={entry.title} path={entry.poster_path} />
        {owner === undefined ? null : (
          <span className="programme-owner">
            <PersonDisc name={owner.name} />
            <span className="sr-only">{owner.name}’s</span>
          </span>
        )}
      </span>
      <span className="programme-text">
        <span className="programme-name">{entry.title}</span>
        <span className="programme-meta">
          <KindGlyph kind={entry.kind} size={14} />
          {entry.status === "completed" && entry.completion_date !== null ? (
            <span className="num">
              <span className="sr-only">Completed </span>
              <time dateTime={entry.completion_date}>
                {formatDay(entry.completion_date)}
              </time>
            </span>
          ) : entry.release_date === null ? null : (
            <span className="num">{entry.release_date.slice(0, 4)}</span>
          )}
        </span>
        {entry.status === "watching" && entry.overview !== null ? (
          <span className="programme-overview">{entry.overview}</span>
        ) : null}
      </span>
    </button>
  )
}
