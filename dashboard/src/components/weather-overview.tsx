import { createElement, Suspense } from "react"
import { Await, Link } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { ClockCounterClockwiseIcon } from "@phosphor-icons/react/dist/csr/ClockCounterClockwise"
import { CloudIcon } from "@phosphor-icons/react/dist/csr/Cloud"
import { CloudSlashIcon } from "@phosphor-icons/react/dist/csr/CloudSlash"
import { CloudFogIcon } from "@phosphor-icons/react/dist/csr/CloudFog"
import { CloudLightningIcon } from "@phosphor-icons/react/dist/csr/CloudLightning"
import { CloudRainIcon } from "@phosphor-icons/react/dist/csr/CloudRain"
import { CloudSnowIcon } from "@phosphor-icons/react/dist/csr/CloudSnow"
import { CloudSunIcon } from "@phosphor-icons/react/dist/csr/CloudSun"
import { DropIcon } from "@phosphor-icons/react/dist/csr/Drop"
import { QuestionIcon } from "@phosphor-icons/react/dist/csr/Question"
import { SunIcon } from "@phosphor-icons/react/dist/csr/Sun"
import { ThermometerSimpleIcon } from "@phosphor-icons/react/dist/csr/ThermometerSimple"
import { UmbrellaIcon } from "@phosphor-icons/react/dist/csr/Umbrella"
import { WindIcon } from "@phosphor-icons/react/dist/csr/Wind"
import type { Icon } from "@phosphor-icons/react/dist/lib/types"
import { CheckAgainButton } from "./check-again-button"
import { LiveWeather } from "./live-weather"
import { weatherDescription, type WeatherRead } from "@/lib/weather"
import { getWeather } from "@/lib/weather.functions"

const number = new Intl.NumberFormat("da-DK", { maximumFractionDigits: 1 })

const timestamp = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  day: "numeric",
  month: "short",
  hour: "2-digit",
  minute: "2-digit",
})

const date = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  weekday: "short",
  day: "numeric",
  month: "short",
})

const hourTime = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
})

const hourOffset = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  timeZoneName: "shortOffset",
})

const weekday = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  weekday: "short",
})

function value(amount: number | null | undefined, unit: string): string {
  return amount === null || amount === undefined
    ? "Unavailable"
    : `${number.format(amount)} ${unit}`
}

/** A figure with its unit set smaller; the text still reads "12,4 °C". */
function Measure({
  amount,
  unit,
}: {
  amount: number | null | undefined
  unit: string
}): React.JSX.Element {
  if (amount === null || amount === undefined)
    return <span data-gap="true">Unavailable</span>

  return (
    <>
      {number.format(amount)} <span className="unit">{unit}</span>
    </>
  )
}

/** WMO code ranges and their glyphs; unknown codes never draw a clear sky. */
const glyphs: readonly (readonly [from: number, to: number, glyph: Icon])[] = [
  [0, 1, SunIcon],
  [2, 2, CloudSunIcon],
  [3, 3, CloudIcon],
  [45, 45, CloudFogIcon],
  [48, 48, CloudFogIcon],
  [51, 67, CloudRainIcon],
  [71, 77, CloudSnowIcon],
  [80, 82, CloudRainIcon],
  [85, 86, CloudSnowIcon],
  [95, 99, CloudLightningIcon],
]

/** A WMO code as a glyph. */
function WeatherGlyph({
  code,
  size,
}: {
  code: number | null
  size: number
}): React.JSX.Element {
  const match = glyphs.find(
    ([from, to]) => code !== null && code >= from && code <= to
  )

  return createElement(match?.[2] ?? QuestionIcon, {
    size,
    "aria-hidden": true,
  })
}

type Days = Extract<WeatherRead, { state: "fresh" }>["forecast"]["days"]

/** Highs and lows on one shared scale, so the week reads as a shape. */
function weekScale(days: Days): (celsius: number) => number {
  const values = days.flatMap((day) =>
    [day.low_c, day.high_c].filter((amount) => amount !== null)
  )

  const low = Math.min(...values)
  const high = Math.max(...values)

  return (celsius) => (high === low ? 0.5 : (celsius - low) / (high - low))
}

function weekdayName(date: string): string {
  return weekday.format(new Date(`${date}T12:00:00Z`)).replace(".", "")
}

function WeatherFailure({
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

/** A reading, or a dash drawn over the word "Unavailable" that assistive technology still reads. */
function Reading({
  amount,
  unit,
  quiet = false,
}: {
  amount: number | null | undefined
  unit: string
  /** Drawn only for assistive technology, such as a dry hour's 0 mm. */
  quiet?: boolean
}): React.JSX.Element {
  if (amount === null || amount === undefined)
    return <span data-gap="true">Unavailable</span>

  return (
    <span className={quiet ? "sr-only" : undefined}>{value(amount, unit)}</span>
  )
}

function HourlyForecast({
  hours,
}: {
  hours: Extract<WeatherRead, { state: "fresh" }>["forecast"]["hourly"]
}): React.JSX.Element {
  const peak = Math.max(1, ...hours.map((hour) => hour.precipitation_mm ?? 0))

  const temperatures = hours
    .map((hour) => hour.temperature_c)
    .filter((amount) => amount !== null)

  const coldest = Math.min(...temperatures)
  const warmest = Math.max(...temperatures)

  const lift = (celsius: number): number =>
    warmest === coldest ? 0.5 : (celsius - coldest) / (warmest - coldest)

  return (
    <section aria-labelledby="hourly-heading">
      <div className="sr-only">
        <h2 id="hourly-heading">Next 24 hours</h2>
        <p>Walking or running?</p>
      </div>
      {hours.length === 0 ? (
        <p className="wx-missing" role="status">
          <CloudSlashIcon size={28} weight="light" aria-hidden="true" />
          <span className="sr-only">Hourly forecast unavailable</span>
        </p>
      ) : (
        <>
          <p id="hourly-help" className="sr-only">
            Compare temperature, precipitation and wind before heading out.
            Scroll across for later hours. Missing values are unavailable.
          </p>
          <div
            className="weather-hourly-scroll"
            role="region"
            aria-label="Hourly conditions"
            aria-describedby="hourly-help"
            tabIndex={0}
          >
            <table className="weather-hourly-table">
              <caption className="sr-only">
                Temperature and wind at the start of each hour. Precipitation
                amount and chance cover the following hour, including rain,
                showers and snow. Bars compare amounts on the same scale.
              </caption>
              <thead>
                <tr>
                  <th scope="col">
                    <span className="sr-only">Hour starting</span>
                  </th>
                  {hours.map((hour, index) => (
                    <th
                      scope="col"
                      key={hour.starts_at}
                      data-upcoming={index < 3}
                    >
                      <span className="weather-hour-day">
                        {date.format(new Date(hour.starts_at))}
                      </span>
                      <time dateTime={hour.starts_at}>
                        {hourTime.format(new Date(hour.starts_at))}
                      </time>
                      <span className="weather-hour-zone">
                        {
                          hourOffset
                            .formatToParts(new Date(hour.starts_at))
                            .find((part) => part.type === "timeZoneName")?.value
                        }
                      </span>
                      {hour.temperature_c === null &&
                      hour.wind_ms === null &&
                      hour.precipitation_mm === null &&
                      hour.precipitation_probability_percent === null ? (
                        <span className="sr-only">
                          Hourly conditions unavailable
                        </span>
                      ) : null}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                <tr className="weather-hour-temperature">
                  <th scope="row">
                    <ThermometerSimpleIcon size={20} aria-hidden="true" />
                    <span className="sr-only">Temperature</span>
                  </th>
                  {hours.map((hour) => (
                    <td key={hour.starts_at}>
                      <span
                        className="weather-hour-lift"
                        data-gap={hour.temperature_c === null}
                        // SAFETY: CSSProperties omits custom properties; --lift is a unitless 0..1 temperature height read by .weather-hour-lift.
                        style={
                          {
                            "--lift":
                              hour.temperature_c === null
                                ? 0
                                : lift(hour.temperature_c),
                          } as React.CSSProperties
                        }
                      >
                        {value(hour.temperature_c, "°C")}
                      </span>
                    </td>
                  ))}
                </tr>
                <tr className="weather-hour-rain">
                  <th scope="row">
                    <DropIcon size={20} aria-hidden="true" />
                    <span className="sr-only">
                      Precipitation <span>Over this hour</span>
                    </span>
                  </th>
                  {hours.map((hour) => (
                    <td key={hour.starts_at}>
                      <div
                        className="weather-rain-track"
                        aria-hidden="true"
                        data-missing={hour.precipitation_mm === null}
                      >
                        {hour.precipitation_mm !== null ? (
                          <svg
                            className="weather-rain-bar"
                            viewBox="0 0 24 48"
                            aria-hidden="true"
                          >
                            <rect
                              x="0"
                              y={48 - (hour.precipitation_mm / peak) * 48}
                              width="24"
                              height={(hour.precipitation_mm / peak) * 48}
                              rx="3"
                            />
                          </svg>
                        ) : null}
                      </div>
                      <Reading
                        amount={hour.precipitation_mm}
                        unit="mm"
                        quiet={hour.precipitation_mm === 0}
                      />
                    </td>
                  ))}
                </tr>
                <tr className="weather-hour-quiet">
                  <th scope="row">
                    <UmbrellaIcon size={20} aria-hidden="true" />
                    <span className="sr-only">
                      Chance <span>Of precipitation</span>
                    </span>
                  </th>
                  {hours.map((hour) => (
                    <td key={hour.starts_at}>
                      <Reading
                        amount={hour.precipitation_probability_percent}
                        unit="%"
                        quiet={
                          (hour.precipitation_probability_percent ?? 100) < 20
                        }
                      />
                    </td>
                  ))}
                </tr>
                <tr className="weather-hour-quiet">
                  <th scope="row">
                    <WindIcon size={20} aria-hidden="true" />
                    <span className="sr-only">Wind</span>
                  </th>
                  {hours.map((hour) => (
                    <td key={hour.starts_at}>
                      <Reading amount={hour.wind_ms} unit="m/s" />
                    </td>
                  ))}
                </tr>
              </tbody>
            </table>
          </div>
        </>
      )}
    </section>
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

/** A whole-degree reading, or a dash when the forecast has none. */
function degrees(celsius: number | null | undefined): string {
  return celsius === null || celsius === undefined
    ? "–"
    : `${Math.round(celsius)}°`
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

/** Whether the forecast still holds: nothing to see while fresh, a rewind mark when stale. */
export function WeatherFreshness({
  weather,
}: {
  weather: WeatherRead
}): React.JSX.Element | null {
  if (weather.state === "unavailable") return null

  const stale = weather.state === "stale"
  const fetched = new Date(weather.forecast.fetched_at)

  return (
    <div className="weather-freshness" role="status">
      {stale ? (
        <span className="weather-stale-mark" aria-hidden="true">
          <ClockCounterClockwiseIcon size={20} weight="bold" />
          <time dateTime={weather.forecast.fetched_at}>
            {hourTime.format(fetched)}
          </time>
        </span>
      ) : null}
      <span className="sr-only">
        {stale ? "Stale forecast" : "Fresh forecast"}. Fetched{" "}
        {timestamp.format(fetched)} · Valid until{" "}
        {timestamp.format(new Date(weather.forecast.valid_until))}
      </span>
      <CheckAgainButton compact />
    </div>
  )
}

/** Current conditions, the next day by the hour and seven days, from one cached forecast. */
export function WeatherOverview({
  weather,
}: {
  weather: WeatherRead
}): React.JSX.Element {
  if (weather.state === "unavailable")
    return <WeatherFailure reason={weather.reason} />

  const { forecast } = weather
  const current = forecast.current
  const scale = weekScale(forecast.days)

  return (
    <div className="wx" data-stale={weather.state === "stale"}>
      {weather.state === "stale" ? (
        <p className="sr-only">
          Showing the last cached forecast; conditions may have changed.
        </p>
      ) : null}
      <section className="wx-now" aria-labelledby="current-heading">
        <h2 id="current-heading" className="sr-only">
          Current conditions
        </h2>
        <p className="wx-temp">
          <Measure amount={current?.temperature_c} unit="°C" />
        </p>
        <p className="wx-condition">
          <WeatherGlyph code={current?.weather_code ?? null} size={112} />
          <span className="sr-only">
            {weatherDescription(current?.weather_code ?? null)}
          </span>
        </p>
        <dl className="wx-measures">
          <div>
            <dt>
              <ThermometerSimpleIcon size={22} aria-hidden="true" />
              <span className="sr-only">Feels like</span>
            </dt>
            <dd>
              <Reading amount={current?.feels_like_c} unit="°C" />
            </dd>
          </div>
          <div>
            <dt>
              <WindIcon size={22} aria-hidden="true" />
              <span className="sr-only">Wind</span>
            </dt>
            <dd>
              <Reading amount={current?.wind_ms} unit="m/s" />
            </dd>
          </div>
        </dl>
        <p className="sr-only">
          {current === null
            ? "Current conditions unavailable"
            : `Modelled at ${timestamp.format(new Date(current.observed_at))}`}
        </p>
      </section>
      <HourlyForecast hours={forecast.hourly} />
      <section aria-labelledby="forecast-heading">
        <h2 id="forecast-heading" className="sr-only">
          Seven days
        </h2>
        <ol className="wx-days">
          {forecast.days.map((day) => (
            <li key={day.date} className="wx-day">
              <h3>
                <time dateTime={day.date}>
                  <span aria-hidden="true">{weekdayName(day.date)}</span>
                  <span className="sr-only">
                    {date.format(new Date(`${day.date}T12:00:00Z`))}
                  </span>
                </time>
              </h3>
              <p className="wx-day-condition">
                <WeatherGlyph code={day.weather_code} size={30} />
                <span className="sr-only">
                  {weatherDescription(day.weather_code)}
                </span>
              </p>
              <dl>
                <div className="wx-day-high" data-gap={day.high_c === null}>
                  <dt className="sr-only">High</dt>
                  <dd>{value(day.high_c, "°C")}</dd>
                </div>
                <div className="wx-range" aria-hidden="true">
                  {day.low_c === null || day.high_c === null ? null : (
                    <span
                      // SAFETY: CSSProperties omits custom properties; --low and --high are unitless 0..1 shares of the week's temperature scale.
                      style={
                        {
                          "--low": scale(day.low_c),
                          "--high": scale(day.high_c),
                        } as React.CSSProperties
                      }
                    />
                  )}
                </div>
                <div className="wx-day-low" data-gap={day.low_c === null}>
                  <dt className="sr-only">Low</dt>
                  <dd>{value(day.low_c, "°C")}</dd>
                </div>
                <div
                  className="wx-day-rain"
                  data-dry={day.precipitation_mm === 0}
                  data-gap={day.precipitation_mm === null}
                >
                  <dt className="sr-only">Precipitation</dt>
                  <dd>{value(day.precipitation_mm, "mm")}</dd>
                </div>
                <div className="wx-day-wind" data-gap={day.wind_ms === null}>
                  <dt className="sr-only">Max wind</dt>
                  <dd>{value(day.wind_ms, "m/s")}</dd>
                </div>
              </dl>
            </li>
          ))}
        </ol>
      </section>
      <p className="weather-attribution">
        Weather data by{" "}
        <a href="https://open-meteo.com/" target="_blank" rel="noreferrer">
          Open-Meteo
        </a>
        , licensed under{" "}
        <a
          href="https://creativecommons.org/licenses/by/4.0/"
          target="_blank"
          rel="noreferrer"
        >
          CC BY 4.0
        </a>
        .<span className="sr-only"> Forecasts can change.</span>
      </p>
    </div>
  )
}
