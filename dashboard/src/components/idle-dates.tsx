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

export type IdleDatesLead =
  | React.ReactNode
  | ((dates: readonly ImportantDate[]) => React.ReactNode)

export function IdleDates({
  lead,
  read = getIdleDates,
}: {
  lead?: IdleDatesLead
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
      () => {}
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
    <div
      data-slot="idle-dates"
      className="grid max-w-120 gap-idle-dates idle-short:gap-2"
    >
      {leading === undefined || leading === null ? null : (
        <div data-slot="idle-dates-lead" className="empty:hidden">
          {leading}
        </div>
      )}
      {since.map((date) => (
        <div className="min-w-0" key={date.id}>
          <p className="text-idle-since-title text-muted-ink">{date.title}</p>
          <p
            data-slot="idle-since-figure"
            className="text-idle-since leading-idle-snug font-medium num idle-short:text-idle-since-short"
          >
            {figureParts(date).map((part, at) => (
              <span className="inline-block" key={part}>
                {at > 0 ? (
                  <span className="mx-idle-separator" aria-hidden="true">
                    {" · "}
                  </span>
                ) : null}
                {part}
              </span>
            ))}
          </p>
          {date.elapsed === null ? null : (
            <p className="text-idle-note num text-muted-ink">
              {date.elapsed.total_days.toLocaleString("en-GB")}{" "}
              {date.elapsed.total_days === 1 ? "day" : "days"}
            </p>
          )}
        </div>
      ))}
      {upcoming === undefined ? null : (
        <p
          className="text-idle-upcoming leading-idle-tight num"
          key={upcoming.id}
        >
          {upcoming.title} {countdown(upcoming.next.days_until).toLowerCase()}
        </p>
      )}
    </div>
  )
}
