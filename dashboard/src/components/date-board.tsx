import { useEffect, useState } from "react"
import { cva } from "class-variance-authority"
import { CalendarHeartIcon } from "@phosphor-icons/react/dist/csr/CalendarHeart"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { OpenConnection } from "@/components/control-alert"
import {
  DateAdd,
  DateDetail,
  DateFigure,
  DateKindGlyph,
} from "@/components/date-sheet"
import {
  BoardHeading,
  ListBar,
  ListBarEnd,
  ListEmpty,
  ListsFrame,
} from "@/components/list-frame"
import {
  FeedbackLine,
  ListSheet,
  PersonDisc,
  PersonKeys,
  useListSave,
} from "@/components/list-parts"
import { Button } from "@/components/ui/button"
import {
  copenhagenDay,
  datesFor,
  importantDateFailureMessage,
  type ImportantDate,
  type ImportantDateCommand,
  type ImportantDateMutation,
  type ImportantDatesState,
} from "@/lib/important-dates"
import { formatDay, type PersonSearch } from "@/lib/lists"
import { cn } from "@/lib/utils"
import type { HouseholdUser } from "@/lib/wishlist"

type Open = { kind: "date"; id: string } | { kind: "add" }

const groups = [
  { kind: "since", label: "Since" },
  { kind: "yearly", label: "Every year" },
] as const satisfies ReadonlyArray<{
  kind: ImportantDate["kind"]
  label: string
}>

export function DateBoard({
  data,
  search,
  choose,
  change,
  refresh,
  lead,
}: {
  data: ImportantDatesState
  search: PersonSearch
  choose: (search: PersonSearch) => void
  change: (command: ImportantDateCommand) => Promise<ImportantDateMutation>
  refresh: () => Promise<void>
  lead?: React.ReactNode
}): React.JSX.Element {
  const { busy, feedback, save } = useListSave(
    change,
    refresh,
    importantDateFailureMessage,
    "unreachable"
  )

  const [open, setOpen] = useState<Open>()
  const on = data.state === "available" ? data.on : undefined

  useEffect(() => {
    if (on === undefined) return

    const timer = setInterval(() => {
      if (
        document.visibilityState !== "hidden" &&
        copenhagenDay(new Date()) !== on
      )
        void refresh().catch(() => undefined)
    }, 60_000)

    return () => clearInterval(timer)
  }, [on, refresh])

  if (data.state === "unavailable")
    return (
      <ListsFrame>
        {lead === undefined ? null : <ListBar>{lead}</ListBar>}
        <ListEmpty icon={CalendarHeartIcon} title="Dates unavailable">
          <p>{importantDateFailureMessage[data.reason]}</p>
          <OpenConnection />
        </ListEmpty>
      </ListsFrame>
    )

  const { owners, dates } = data
  const owner = owners.find((candidate) => candidate.id === search.person)
  const shown = datesFor(dates, owner?.id)
  const counts = new Map<string, number>()

  for (const candidate of owners)
    counts.set(candidate.id, datesFor(dates, candidate.id).length)

  const selected =
    open?.kind === "date"
      ? dates.find((date) => date.id === open.id)
      : undefined

  return (
    <ListsFrame aria-busy={busy}>
      <ListBar>
        {lead}
        {owners.length === 0 ? null : (
          <PersonKeys
            owners={owners}
            selected={owner?.id}
            counts={counts}
            noun={["date", "dates"]}
            label="Whose dates"
            choose={(person) => choose({ person })}
          />
        )}
        <ListBarEnd>
          <Button size="key" onClick={() => setOpen({ kind: "add" })}>
            <PlusIcon size={20} weight="bold" aria-hidden="true" />
            Add a date
          </Button>
        </ListBarEnd>
      </ListBar>
      {open === undefined ? (
        <FeedbackLine feedback={feedback} busy={busy} />
      ) : null}
      {shown.length === 0 ? (
        <ListEmpty icon={CalendarHeartIcon} title="No dates yet">
          <p>
            Add the days you count from, like when you met, and the ones that
            come round every year.
          </p>
        </ListEmpty>
      ) : (
        <div className="grid gap-dates">
          {groups.map((group) => {
            const items = shown.filter((date) => date.kind === group.kind)

            if (items.length === 0) return null

            return (
              <section
                key={group.kind}
                className="grid gap-3.5"
                data-kind={group.kind}
                aria-label={group.label}
              >
                <BoardHeading count={items.length} noun={["date", "dates"]}>
                  {group.label}
                </BoardHeading>
                <ul
                  data-slot="date-plates"
                  className={plates({ kind: group.kind })}
                >
                  {items.map((date) => (
                    <li key={date.id} className="grid">
                      <DatePlate
                        date={date}
                        people={
                          owner === undefined
                            ? date.person_ids
                                .map((id) =>
                                  owners.find(
                                    (candidate) => candidate.id === id
                                  )
                                )
                                .filter((person) => person !== undefined)
                            : []
                        }
                        open={() => setOpen({ kind: "date", id: date.id })}
                      />
                    </li>
                  ))}
                </ul>
              </section>
            )
          })}
        </div>
      )}
      <ListSheet
        open={selected !== undefined}
        close={() => setOpen(undefined)}
        title={selected?.title ?? "Date"}
        titleHidden
      >
        {selected === undefined ? null : (
          <>
            <DateDetail
              key={selected.id}
              date={selected}
              owners={owners}
              busy={busy}
              save={save}
              done={() => setOpen(undefined)}
            />
            <FeedbackLine feedback={feedback} busy={busy} />
          </>
        )}
      </ListSheet>
      <ListSheet
        open={open?.kind === "add"}
        close={() => setOpen(undefined)}
        title="Add a date"
      >
        {open?.kind === "add" ? (
          <>
            <DateAdd
              owners={owners}
              person={owner?.id}
              busy={busy}
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

const plates = cva("grid gap-plates", {
  variants: {
    kind: { since: "grid-fill-80", yearly: "grid-fill-54" },
  },
})

const plate = cva(
  "grid w-full grid-rows-date gap-2.5 rounded-plate bg-wash px-4.5 pt-3.5 pb-4 text-left transition duration-fast ease-app hover:bg-wash-strong active:scale-98",
  {
    variants: {
      kind: {
        since: "min-h-46 max-sm:min-h-0",
        yearly: "min-h-38 max-sm:min-h-0",
      },
    },
  }
)

function DatePlate({
  date,
  people,
  open,
}: {
  date: ImportantDate
  people: readonly HouseholdUser[]
  open: () => void
}): React.JSX.Element {
  const today = date.next.days_until === 0 || date.milestone !== null

  return (
    <button
      type="button"
      className={plate({ kind: date.kind })}
      data-kind={date.kind}
      data-today={today}
      onClick={open}
    >
      <span className="flex min-w-0 items-center gap-2 text-muted-ink">
        <DateKindGlyph kind={date.kind} size={16} />
        <span
          data-slot="date-plate-title"
          className="truncate text-body-lg font-medium text-ink"
        >
          {date.title}
        </span>
        {today ? (
          <span
            className={cn(
              "size-3 flex-none rounded-full bg-sun",
              people.length === 0 && "ml-auto"
            )}
          >
            <span className="sr-only">Today</span>
          </span>
        ) : null}
        {people.length === 0 ? null : (
          <span className="ml-auto flex flex-none gap-1">
            {people.map((person) => (
              <PersonDisc key={person.id} name={person.name} size="plate" />
            ))}
            <span className="sr-only">
              {people.map((person) => person.name).join(" and ")}
            </span>
          </span>
        )}
      </span>
      <DateFigure date={date} />
      <span className="flex items-center gap-3">
        <span className="flex flex-wrap gap-x-3 gap-y-1 text-control num text-muted-ink">
          <time dateTime={date.date}>{formatDay(date.date)}</time>
          {date.kind === "since" && date.elapsed !== null ? (
            <span>
              {date.elapsed.total_days.toLocaleString("en-GB")}
              {date.elapsed.total_days === 1 ? " day" : " days"}
            </span>
          ) : date.kind === "yearly" && date.next.years > 0 ? (
            <span>
              {date.next.years}
              {date.next.years === 1 ? " year" : " years"}
            </span>
          ) : null}
        </span>
      </span>
    </button>
  )
}
