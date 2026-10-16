import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react"
import { DayRings } from "@/components/score-ring"
import { chargeBand, ringShare } from "@/lib/recovery-score"
import {
  shiftDay,
  stripDays,
  type RecoveryDay,
  type TrainingRead,
  type TrainingState,
} from "@/lib/training"

type Entries = TrainingState["week"]

/** How far back the strip will ever reach; older days are not offered. */
const reach = 364

const weekday = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "narrow",
})

const dayLabel = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "short",
  day: "numeric",
  month: "short",
})

/** Training Days are calendar dates; formatting them at UTC midnight never shifts the day. */
function calendar(day: string, format: Intl.DateTimeFormat): string {
  return format.format(new Date(`${day}T00:00:00Z`))
}

/** One day's three readings, ready to draw as rings. */
type DayReadings = {
  readonly charge: number | null
  readonly rest: number | null
  readonly effort: number | null
}

/** The readings of a Recovery Day read; a failed read has none. */
export function readings(read: TrainingRead<RecoveryDay>): DayReadings {
  if (read.state === "unavailable")
    return { charge: null, rest: null, effort: null }

  return {
    charge: read.data.derived_scores.recovery.value,
    rest: read.data.derived_scores.rest.value,
    effort: read.data.derived_scores.strain.value,
  }
}

function spokenReadings(values: DayReadings): string {
  return [
    `Charge ${values.charge === null ? "not recorded" : `${Math.round(values.charge)} %`}`,
    `Rest ${values.rest === null ? "not recorded" : `${Math.round(values.rest)} %`}`,
    `Effort ${values.effort === null ? "not recorded" : Math.round(values.effort)}`,
  ].join(", ")
}

/** Whether a read brought any ring at all; a whole run without one is before the history began. */
function recorded(entries: Entries): boolean {
  return entries.some((entry) => {
    const values = readings(entry.recovery)

    return (
      values.charge !== null || values.rest !== null || values.effort !== null
    )
  })
}

type DayStripProps = {
  /** The days the page read, oldest first, including the selected one. */
  readonly initial: Entries
  readonly selected: string
  readonly today: string
  readonly disabled: boolean
  /** Select a day, or read the pressed one again. */
  readonly choose: (day: string, current: boolean) => void
  /** Reads more days into the strip; without it only the days given are offered. */
  readonly loadDays?:
    | ((from: string, to: string) => Promise<Entries>)
    | undefined
  readonly selectedRef: React.Ref<HTMLButtonElement>
}

/**
 * Days as a horizontal strip of three-ring buttons: native overflow with snap points, no arrows
 * and no scrollbar. It opens on the selected day and reaches back through the history as it is
 * scrolled. Each day stays a button, so Tab and the arrow keys move along it.
 */
export function DayStrip({
  initial,
  selected,
  today,
  disabled,
  choose,
  loadDays,
  selectedRef,
}: DayStripProps): React.JSX.Element {
  const [entries, setEntries] = useState(initial)
  const [exhausted, setExhausted] = useState(loadDays === undefined)
  const strip = useRef<HTMLOListElement>(null)
  const loading = useRef(false)
  const anchor = useRef<{ width: number; left: number } | undefined>(undefined)

  const first = entries[0]?.day
  const last = entries.at(-1)?.day

  // Land on the selected day, and again whenever the strip changes size or its fonts settle;
  // for today the strip rests against its right end.
  useLayoutEffect(() => {
    const list = strip.current

    if (list === null) return

    const align = () => {
      const button = list.querySelector<HTMLElement>('[aria-pressed="true"]')

      if (button === null) return
      list.scrollLeft =
        selected >= today
          ? list.scrollWidth
          : button.offsetLeft - (list.clientWidth - button.offsetWidth) / 2
    }

    align()

    const observer = new ResizeObserver(align)

    observer.observe(list)

    return () => observer.disconnect()
  }, [selected, today])

  // Days older than the page read keep the view where it was.
  useLayoutEffect(() => {
    const list = strip.current

    if (list === null || anchor.current === undefined) return
    list.scrollLeft =
      anchor.current.left + list.scrollWidth - anchor.current.width
    anchor.current = undefined
  }, [first])

  const extend = useCallback(
    async (from: string, to: string): Promise<Entries | undefined> => {
      if (loadDays === undefined || loading.current) return undefined
      loading.current = true

      try {
        return await loadDays(from, to)
      } catch {
        return undefined
      } finally {
        loading.current = false
      }
    },
    [loadDays]
  )

  // A selected day in the past leaves days up to today unread.
  useEffect(() => {
    if (last === undefined || last >= today) return

    void extend(shiftDay(last, 1), today).then((more) => {
      if (more !== undefined) setEntries((now) => [...now, ...more])
    })
  }, [extend, last, today])

  async function older(): Promise<void> {
    if (first === undefined || exhausted) return

    const earliest = shiftDay(today, -reach)
    const to = shiftDay(first, -1)

    if (to < earliest) {
      setExhausted(true)

      return
    }

    const from = shiftDay(to, 1 - stripDays)
    const more = await extend(from < earliest ? earliest : from, to)

    if (more === undefined) return

    if (!recorded(more)) setExhausted(true)

    const list = strip.current

    if (list !== null)
      anchor.current = { width: list.scrollWidth, left: list.scrollLeft }
    setEntries((now) => [...more, ...now])
  }

  function onKeyDown(event: React.KeyboardEvent<HTMLOListElement>): void {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return

    const buttons = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>("button")
    )

    const at = buttons.findIndex((button) => button === document.activeElement)

    if (at === -1) return
    event.preventDefault()
    buttons[at + (event.key === "ArrowLeft" ? -1 : 1)]?.focus()
  }

  return (
    <ol
      ref={strip}
      className="today-days"
      aria-label="Training days"
      onKeyDown={onKeyDown}
      onScroll={(event) => {
        if (event.currentTarget.scrollLeft < 160) void older()
      }}
    >
      {entries.map((entry) => {
        const values = readings(entry.recovery)
        const current = entry.day === selected

        return (
          <li key={entry.day}>
            <button
              ref={current ? selectedRef : undefined}
              type="button"
              className="today-day"
              aria-pressed={current}
              data-today={entry.day === today}
              disabled={disabled}
              aria-label={`${calendar(entry.day, dayLabel)}: ${spokenReadings(values)}`}
              onClick={() => choose(entry.day, current)}
            >
              <span className="today-day-name" aria-hidden="true">
                {calendar(entry.day, weekday)}
              </span>
              <DayRings
                charge={{
                  share: ringShare(values.charge, 100),
                  band: chargeBand(values.charge),
                }}
                rest={{
                  share: ringShare(values.rest, 100),
                  band: chargeBand(values.rest),
                }}
                effort={{ share: ringShare(values.effort, 100) }}
              />
              <span className="today-day-date" aria-hidden="true">
                {Number(entry.day.slice(8))}
              </span>
            </button>
          </li>
        )
      })}
    </ol>
  )
}
