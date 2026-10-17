import { useEffect, useState } from "react"
import { DWELL_MS } from "@/components/idle-screen"
import {
  figureParts,
  countdown,
  type ImportantDatesState,
  type ImportantDate,
} from "@/lib/important-dates"
import { getIdleDates } from "@/lib/important-dates.functions"
import { useCopenhagenDay } from "@/lib/use-copenhagen-day"

/** Optional lead content, or a render slot sharing the filtered dates and their midnight refresh. */
export type IdleDatesLead =
  | React.ReactNode
  | ((dates: readonly ImportantDate[]) => React.ReactNode)

/** Important Dates on the clock plate. The optional lead slot holds the milestone line. */
export function IdleDates({
  lead,
  read = getIdleDates,
}: {
  lead?: IdleDatesLead
  /** The authenticated dates read, separated from the room-display lifecycle. */
  read?: () => Promise<ImportantDatesState>
}): React.ReactNode {
  const day = useCopenhagenDay()

  return <IdleDateContent key={day} lead={lead} read={read} />
}

function IdleDateContent({
  lead,
  read,
}: {
  lead: IdleDatesLead
  read: () => Promise<ImportantDatesState>
}): React.ReactNode {
  const [data, setData] = useState<ImportantDatesState>()
  const [rotation, setRotation] = useState(0)

  useEffect(() => {
    let current = true
    void read().then(
      (read) => {
        if (current) setData(read)
      },
      () => {
        /* Keep the clock and weather when the read fails. */
      }
    )

    return () => {
      current = false
    }
  }, [read])

  const dates =
    data?.state === "available"
      ? data.dates.filter((date) => date.show_when_idle)
      : []

  const since = dates.filter((date) => date.kind === "since")

  const yearly = dates
    .filter((date) => date.kind === "yearly")
    .sort((a, b) => a.next.days_until - b.next.days_until)

  const count = yearly.length

  useEffect(() => {
    if (count < 2) return
    const timer = setInterval(() => setRotation((at) => at + 1), DWELL_MS)

    return () => clearInterval(timer)
  }, [count])

  const upcoming = yearly[rotation % Math.max(1, count)]

  // oxlint-disable-next-line anti-slop/no-runtime-typeof -- A typed React render slot, not unparsed input.
  const leading = typeof lead === "function" ? lead(dates) : lead

  if (dates.length === 0) return null

  return (
    <div className="idle-dates">
      {leading === undefined || leading === null ? null : (
        <div className="idle-dates-lead">{leading}</div>
      )}
      {since.map((date) => (
        <div className="idle-since" key={date.id}>
          <p className="idle-since-title">{date.title}</p>
          <p className="idle-since-figure num">
            {figureParts(date).map((part, at) => (
              <span key={part}>
                {at > 0 ? <span aria-hidden="true"> · </span> : null}
                {part}
              </span>
            ))}
          </p>
          {date.elapsed === null ? null : (
            <p className="idle-since-total num">
              {date.elapsed.total_days.toLocaleString("en-GB")}{" "}
              {date.elapsed.total_days === 1 ? "day" : "days"}
            </p>
          )}
        </div>
      ))}
      {upcoming === undefined ? null : (
        <p className="idle-upcoming num" key={upcoming.id}>
          {upcoming.title} {countdown(upcoming.next.days_until).toLowerCase()}
        </p>
      )}
    </div>
  )
}
