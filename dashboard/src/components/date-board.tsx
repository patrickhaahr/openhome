import { useEffect, useState } from "react"
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
import type { HouseholdUser } from "@/lib/wishlist"

type Open = { kind: "date"; id: string } | { kind: "add" }

const groups = [
  { kind: "since", label: "Since" },
  { kind: "yearly", label: "Every year" },
] as const satisfies ReadonlyArray<{
  kind: ImportantDate["kind"]
  label: string
}>

/**
 * The household's Important Dates as plates: what it counts from first, then the yearly dates in
 * the order they come round. Axum computes every figure; the view re-reads when the Copenhagen
 * day turns, so the figures stay live on an always-on display.
 */
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
      <div className="lists">
        {lead === undefined ? null : <div className="lists-bar">{lead}</div>}
        <div className="lists-empty">
          <CalendarHeartIcon size={48} weight="light" aria-hidden="true" />
          <h2>Dates unavailable</h2>
          <p>{importantDateFailureMessage[data.reason]}</p>
          <OpenConnection />
        </div>
      </div>
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
    <div className="lists" aria-busy={busy}>
      <div className="lists-bar">
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
        <div className="lists-bar-end">
          <Button size="key" onClick={() => setOpen({ kind: "add" })}>
            <PlusIcon size={20} weight="bold" aria-hidden="true" />
            Add a date
          </Button>
        </div>
      </div>
      {open === undefined ? (
        <FeedbackLine feedback={feedback} busy={busy} />
      ) : null}
      {shown.length === 0 ? (
        <div className="lists-empty">
          <CalendarHeartIcon size={48} weight="light" aria-hidden="true" />
          <h2>No dates yet</h2>
          <p>
            Add the days you count from, like when you met, and the ones that
            come round every year.
          </p>
        </div>
      ) : (
        <div className="dates">
          {groups.map((group) => {
            const items = shown.filter((date) => date.kind === group.kind)

            if (items.length === 0) return null

            return (
              <section
                key={group.kind}
                className="dates-group"
                data-kind={group.kind}
                aria-label={group.label}
              >
                <h2 className="board-heading">
                  {group.label}
                  <span className="board-count num">
                    {items.length}
                    <span className="sr-only">
                      {" "}
                      {items.length === 1 ? "date" : "dates"}
                    </span>
                  </span>
                </h2>
                <ul className="date-plates">
                  {items.map((date) => (
                    <li key={date.id}>
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
    </div>
  )
}

/** A date's plate: its title, the live figure large, then the date and its people. */
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
      className="date-plate"
      data-kind={date.kind}
      data-today={today}
      onClick={open}
    >
      <span className="date-plate-head">
        <DateKindGlyph kind={date.kind} size={16} />
        <span className="date-plate-title">{date.title}</span>
        {today ? (
          <span className="date-mark">
            <span className="sr-only">Today</span>
          </span>
        ) : null}
        {people.length === 0 ? null : (
          <span className="date-plate-people">
            {people.map((person) => (
              <PersonDisc key={person.id} name={person.name} />
            ))}
            <span className="sr-only">
              {people.map((person) => person.name).join(" and ")}
            </span>
          </span>
        )}
      </span>
      <DateFigure date={date} />
      <span className="date-plate-foot">
        <span className="date-plate-meta num">
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
