import { useEffect, useState } from "react"
import { LiveWeather } from "@/components/live-weather"
import { timeFormat } from "@/components/live-clock"
import { degrees, WeatherGlyph } from "@/components/weather-figures"
import { useMinute } from "@/lib/use-minute"
import {
  unavailableWeather,
  weatherDescription,
  type WeatherRead,
} from "@/lib/weather"

const dateFormat = new Intl.DateTimeFormat("en-GB", {
  weekday: "long",
  day: "numeric",
  month: "long",
  timeZone: "Europe/Copenhagen",
})

/**
 * The tone plate in the corner: a large clock, the date and today's weather. `children` is the
 * slot for later idle content (the important-dates counter), set beneath them on the same plate.
 */
export function IdleOverlay({
  weather,
  children,
  yearsAgo,
}: {
  weather: () => Promise<WeatherRead>
  children?: React.ReactNode
  yearsAgo?: number | undefined
}): React.JSX.Element {
  const [openedAt] = useState(() => new Date().toISOString())
  const now = useMinute(openedAt)

  return (
    <section
      className="group/plate absolute bottom-margin left-margin z-2 mb-safe-bottom grid max-w-idle-plate justify-items-start gap-stack rounded-lg bg-ground/50 px-idle-plate-x py-idle-plate-y idle-short:has-[[data-slot=idle-milestone]]:py-3"
      aria-label="Now"
    >
      <time
        data-slot="idle-time"
        className="-ml-title-hang block text-idle-clock leading-idle-clock font-medium num idle-short:group-has-[[data-slot=idle-milestone]]/plate:text-idle-clock-short"
        dateTime={now.toISOString()}
        aria-label="Current time"
      >
        {timeFormat.format(now)}
      </time>
      <p className="mt-1.5 text-idle-date leading-idle-snug font-medium tracking-figure-tight">
        {dateFormat.format(now)}
      </p>
      <IdleWeather read={weather} />
      {yearsAgo === undefined ? null : (
        <p
          data-slot="idle-photo-memory"
          className="text-idle-note num text-muted-ink"
        >
          {yearsAgo} {yearsAgo === 1 ? "year" : "years"} ago today
        </p>
      )}
      {children === undefined || children === null ? null : (
        <div className="mt-2 empty:hidden">{children}</div>
      )}
    </section>
  )
}

/** Today's weather, read once on entering and kept fresh; missing weather simply isn't shown. */
function IdleWeather({
  read,
}: {
  read: () => Promise<WeatherRead>
}): React.ReactNode {
  const [initial, setInitial] = useState<WeatherRead>()

  useEffect(() => {
    let current = true

    void read()
      .catch(() => unavailableWeather)
      .then((weather) => {
        if (current) setInitial(weather)
      })

    return () => {
      current = false
    }
  }, [read])

  if (initial === undefined) return null

  return (
    <LiveWeather initial={initial} readWeather={read}>
      {(weather) => {
        if (weather.state === "unavailable") return null
        const { current, days } = weather.forecast
        const today = days[0]

        return (
          <p className="mt-0.5 flex items-center gap-2 text-idle-date font-medium tracking-figure-tight text-muted-ink">
            <WeatherGlyph
              code={current?.weather_code ?? null}
              size={26}
              className="text-ink"
            />
            <span className="num text-ink" aria-hidden="true">
              {degrees(current?.temperature_c)}
            </span>
            {today === undefined ? null : (
              <span className="text-idle-range num" aria-hidden="true">
                {degrees(today.high_c)} / {degrees(today.low_c)}
              </span>
            )}
            <span className="sr-only">
              {weatherDescription(current?.weather_code ?? null)},{" "}
              {degrees(current?.temperature_c)}
              {today === undefined
                ? ""
                : `, high ${degrees(today.high_c)}, low ${degrees(today.low_c)}`}
            </span>
          </p>
        )
      }}
    </LiveWeather>
  )
}
