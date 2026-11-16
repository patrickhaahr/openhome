import { useMinute } from "@/lib/use-minute"

export const timeFormat = new Intl.DateTimeFormat("en-GB", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
  timeZone: "Europe/Copenhagen",
})

const dateFormat = new Intl.DateTimeFormat("en-GB", {
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
  timeZone: "Europe/Copenhagen",
})

const shortDate = new Intl.DateTimeFormat("en-GB", {
  weekday: "long",
  day: "numeric",
  month: "long",
  timeZone: "Europe/Copenhagen",
})

type LiveClockProps = { readonly initialTime: string }

/** The Home clock: the time as the largest thing in the room, the day beneath it. */
export function LiveClock({ initialTime }: LiveClockProps): React.JSX.Element {
  const now = useMinute(initialTime)

  return (
    <div>
      <time
        className="-ml-clock-hang block text-clock-time leading-figure-xl font-medium tracking-clock tabular-nums"
        dateTime={now.toISOString()}
        aria-label="Current time"
      >
        {timeFormat.format(now)}
      </time>
      <p className="mt-5 text-clock-date leading-workout-heading font-medium tracking-figure-tight first-letter:uppercase">
        <span aria-hidden="true">{shortDate.format(now)}</span>
        <span className="sr-only">{dateFormat.format(now)}</span>
      </p>
      <p className="sr-only">Copenhagen time</p>
    </div>
  )
}
