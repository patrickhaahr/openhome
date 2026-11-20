import { Suspense } from "react"
import { Await, Link } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { ClockCounterClockwiseIcon } from "@phosphor-icons/react/dist/csr/ClockCounterClockwise"
import { CloudSlashIcon } from "@phosphor-icons/react/dist/csr/CloudSlash"
import { CheckAgainButton } from "./check-again-button"
import { LiveWeather } from "./live-weather"
import {
  degrees,
  value,
  WeatherGlyph,
  weekdayName,
  type Days,
} from "./weather-figures"
import { weatherDescription, type WeatherRead } from "@/lib/weather"
import { getWeather } from "@/lib/weather.functions"

export function WeatherFailure({
  reason,
}: {
  reason: "missing-configuration" | "provider-unavailable"
}): React.JSX.Element {
  return (
    <div
      className="grid grid-cols-flanked items-center gap-4 text-muted-ink"
      role="status"
    >
      <CloudSlashIcon size={44} weight="light" aria-hidden="true" />
      <h3 className="text-lead leading-workout-heading font-medium tracking-figure-tight text-ink">
        {reason === "missing-configuration"
          ? "Weather setup needed"
          : "Weather unavailable"}
      </h3>
      <p className="sr-only">
        {reason === "missing-configuration"
          ? "Set the private forecast location on OpenHome to see local conditions."
          : "OpenHome could not provide a forecast. Check Connection or try again."}
      </p>
      <CheckAgainButton compact />
    </div>
  )
}

export function WeatherNow({
  weather,
}: {
  weather: Promise<WeatherRead>
}): React.JSX.Element {
  return (
    <div className="grid gap-5 max-tablet:col-start-2 max-tablet:row-start-1 max-phone:col-auto max-phone:row-auto">
      <Suspense
        fallback={
          <>
            <p role="status" className="sr-only">
              Checking weather…
            </p>
            <span
              className="block h-18 w-36 rounded-lg bg-wash-strong"
              aria-hidden="true"
            />
            <OpenWeather />
          </>
        }
      >
        <Await promise={weather}>
          {(initial) => (
            <LiveWeather initial={initial} readWeather={getWeather}>
              {(read) => {
                if (read.state === "unavailable")
                  return (
                    <>
                      <WeatherFailure reason={read.reason} />
                      <OpenWeather />
                    </>
                  )

                const { current, days } = read.forecast

                return (
                  <Link
                    to="/weather"
                    aria-label="Open Weather"
                    className="group/forecast grid gap-6 text-inherit no-underline"
                    data-stale={read.state === "stale"}
                  >
                    <span className="flex items-center gap-4">
                      <span className="grid text-ink transition-lift duration-mid ease-app group-hover/forecast:-translate-y-1">
                        <WeatherGlyph
                          code={current?.weather_code ?? null}
                          size={48}
                        />
                      </span>
                      <span className="text-forecast-now leading-forecast-now font-medium tracking-forecast-now tabular-nums">
                        {degrees(current?.temperature_c)}
                      </span>
                      {read.state === "stale" ? (
                        <span
                          className="self-start text-danger"
                          aria-hidden="true"
                        >
                          <ClockCounterClockwiseIcon size={18} weight="bold" />
                        </span>
                      ) : null}
                    </span>
                    <span className="sr-only">
                      {weatherDescription(current?.weather_code ?? null)}.{" "}
                      {read.state === "stale" ? "Stale. " : ""}Feels{" "}
                      {value(current?.feels_like_c, "°C")}, wind{" "}
                      {value(current?.wind_ms, "m/s")}.
                    </span>
                    <ForecastDays days={days} />
                  </Link>
                )
              }}
            </LiveWeather>
          )}
        </Await>
      </Suspense>
    </div>
  )
}

function OpenWeather(): React.JSX.Element {
  return (
    <Link
      to="/weather"
      aria-label="Open Weather"
      className="grid size-11 place-items-center rounded-full text-ink transition-colors duration-fast ease-app hover:bg-ink hover:text-ground"
    >
      <ArrowUpRightIcon size={18} aria-hidden="true" />
    </Link>
  )
}

function ForecastDays({ days }: { days: Days }): React.JSX.Element {
  return (
    <ol className="grid grid-cols-7" aria-label="Seven days">
      {days.map((day, index) => (
        <li
          key={day.date}
          className="group/day grid justify-items-center gap-2.5 text-muted-ink"
        >
          <span className="text-label font-medium group-first/day:text-ink">
            {index === 0 ? "Today" : weekdayName(day.date)}
          </span>
          <WeatherGlyph
            code={day.weather_code}
            size={28}
            className="text-ink"
          />
          <span className="sr-only">
            {weatherDescription(day.weather_code)}.
          </span>
          <span className="text-body-lg font-medium text-ink tabular-nums">
            <span className="sr-only">High </span>
            {degrees(day.high_c)}
          </span>
        </li>
      ))}
    </ol>
  )
}
