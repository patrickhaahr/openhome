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
    <div className="weather-empty" role="status">
      <CloudSlashIcon size={44} weight="light" aria-hidden="true" />
      <h3>
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

/** Home weather: the temperature as a figure, then the week as a shape. */
export function WeatherNow({
  weather,
}: {
  weather: Promise<WeatherRead>
}): React.JSX.Element {
  return (
    <div className="weather-now">
      <Suspense
        fallback={
          <>
            <p role="status" className="sr-only">
              Checking weather…
            </p>
            <span className="weather-ghost" aria-hidden="true" />
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
                    className="forecast"
                    data-stale={read.state === "stale"}
                  >
                    <span className="forecast-now">
                      <span className="forecast-now-glyph">
                        <WeatherGlyph
                          code={current?.weather_code ?? null}
                          size={48}
                        />
                      </span>
                      <span className="forecast-now-temp">
                        {degrees(current?.temperature_c)}
                      </span>
                      {read.state === "stale" ? (
                        <span className="forecast-now-stale" aria-hidden="true">
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
    <Link to="/weather" aria-label="Open Weather" className="dir-go">
      <ArrowUpRightIcon size={18} aria-hidden="true" />
    </Link>
  )
}

/** Today and the days ahead: name, glyph and the day's high. Lows live on Weather. */
function ForecastDays({ days }: { days: Days }): React.JSX.Element {
  return (
    <ol className="forecast-days" aria-label="Seven days">
      {days.map((day, index) => (
        <li key={day.date}>
          <span className="forecast-day-name">
            {index === 0 ? "Today" : weekdayName(day.date)}
          </span>
          <WeatherGlyph code={day.weather_code} size={28} />
          <span className="sr-only">
            {weatherDescription(day.weather_code)}.
          </span>
          <span className="forecast-day-high">
            <span className="sr-only">High </span>
            {degrees(day.high_c)}
          </span>
        </li>
      ))}
    </ol>
  )
}
