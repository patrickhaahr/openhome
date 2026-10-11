import { useEffect, useState } from "react"

const timeFormat = new Intl.DateTimeFormat("da-DK", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
  timeZone: "Europe/Copenhagen",
})

const dateFormat = new Intl.DateTimeFormat("da-DK", {
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
  timeZone: "Europe/Copenhagen",
})

type LiveClockProps = { readonly initialTime: string }

/** Hydrates from the rendered time, then follows minute boundaries while visible. */
export function LiveClock({ initialTime }: LiveClockProps): React.JSX.Element {
  const [now, setNow] = useState(() => new Date(initialTime))

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined

    function refresh(): void {
      clearTimeout(timer)

      if (document.visibilityState === "hidden") return

      setNow(new Date())
      timer = setTimeout(refresh, 60_000 - (Date.now() % 60_000))
    }

    refresh()
    document.addEventListener("visibilitychange", refresh)

    return () => {
      clearTimeout(timer)
      document.removeEventListener("visibilitychange", refresh)
    }
  }, [])

  return (
    <div>
      <time
        className="clock-time"
        dateTime={now.toISOString()}
        aria-label="Current time"
        lang="da"
      >
        {timeFormat.format(now)}
      </time>
      <p className="clock-date" lang="da">
        {dateFormat.format(now)}
      </p>
      <p className="clock-zone">Copenhagen time</p>
    </div>
  )
}
