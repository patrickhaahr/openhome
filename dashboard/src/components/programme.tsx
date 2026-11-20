import { useState } from "react"
import { cva } from "class-variance-authority"
import { FilmSlateIcon } from "@phosphor-icons/react/dist/csr/FilmSlate"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Link } from "@tanstack/react-router"
import { OpenConnection } from "@/components/control-alert"
import {
  BoardHeading,
  ListBar,
  ListBarEnd,
  ListEmpty,
  ListsFrame,
  keyLink,
} from "@/components/list-frame"
import {
  FeedbackLine,
  ListSheet,
  PersonDisc,
  PersonKeys,
  useListSave,
} from "@/components/list-parts"
import { mediaMeta } from "@/components/sheet-parts"
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
      <ListsFrame>
        {lead === undefined ? null : <ListBar>{lead}</ListBar>}
        <ListEmpty icon={FilmSlateIcon} title="Watchlists unavailable">
          <p>{watchlistFailureMessage[data.reason]}</p>
          <OpenConnection />
        </ListEmpty>
      </ListsFrame>
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
    <ListsFrame aria-busy={busy}>
      <ListBar>
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
        <ListBarEnd>
          <Button
            size="key"
            disabled={owners.length === 0}
            onClick={() => setOpen({ kind: "add" })}
          >
            <PlusIcon size={20} weight="bold" aria-hidden="true" />
            Add a title
          </Button>
        </ListBarEnd>
      </ListBar>
      {open === undefined ? (
        <FeedbackLine feedback={feedback} busy={busy} />
      ) : null}
      {owners.length === 0 ? (
        <ListEmpty icon={FilmSlateIcon} title="No one here yet">
          <p>Household members are added under Wish.</p>
          <Link to="/lists" search={{ tab: "wish" }} className={keyLink}>
            Open wishes
          </Link>
        </ListEmpty>
      ) : (
        <div className="grid gap-board">
          {rows.map((row) => {
            const items = shown.filter((entry) => entry.status === row.status)

            if (row.status === "completed")
              items.sort((a, b) =>
                (b.completion_date ?? "").localeCompare(a.completion_date ?? "")
              )

            return (
              <section
                key={row.status}
                className="grid gap-3.5"
                data-status={row.status}
                aria-label={row.label}
              >
                <BoardHeading count={items.length} noun={["title", "titles"]}>
                  {row.label}
                </BoardHeading>
                {items.length === 0 ? (
                  <p className="text-muted-ink">
                    {row.status === "watching"
                      ? "Nothing on right now."
                      : row.status === "planned"
                        ? "Nothing planned yet."
                        : "Nothing finished yet."}
                  </p>
                ) : (
                  <ul className={titles({ status: row.status })}>
                    {items.map((entry) => (
                      <li key={entry.id} className="grid">
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
    </ListsFrame>
  )
}

const titles = cva("grid gap-titles", {
  variants: {
    status: {
      watching: "grid-cols-now",
      planned: "grid-fill-35 max-sm:grid-fill-26",
      completed: "grid-fill-27 max-sm:grid-fill-21",
    },
  },
})

const programmeTitle = cva(
  "group/title grid w-full text-left transition duration-fast ease-app active:scale-98",
  {
    variants: {
      status: {
        watching:
          "grid-aside-25 items-start gap-4.5 rounded-tile bg-tile p-2.5 hover:bg-wash-strong max-sm:grid-aside-26 max-sm:gap-4",
        planned: "content-start gap-2.5",
        completed: "content-start gap-2.5",
      },
    },
  }
)

const programmeName = cva("line-clamp-2 font-medium", {
  variants: {
    status: {
      watching: "pt-1 text-programme leading-heading tracking-title",
      planned: "text-control leading-tight",
      completed: "text-small leading-tight text-muted-ink",
    },
  },
})

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
    <button
      type="button"
      data-slot="programme-title"
      className={programmeTitle({ status: entry.status })}
      onClick={open}
    >
      <WatchlistPoster
        title={entry.title}
        path={entry.poster_path}
        chip={
          owner === undefined ? undefined : (
            <>
              <PersonDisc name={owner.name} tone="ground" />
              <span className="sr-only">{owner.name}’s</span>
            </>
          )
        }
      />
      <span className="grid content-start gap-1">
        <span
          data-slot="programme-name"
          className={programmeName({ status: entry.status })}
        >
          {entry.title}
        </span>
        <span className={mediaMeta}>
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
          <span className="mt-1 line-clamp-3 text-control leading-prose text-muted-ink">
            {entry.overview}
          </span>
        ) : null}
      </span>
    </button>
  )
}
