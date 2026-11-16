import { ClockCounterClockwiseIcon } from "@phosphor-icons/react/dist/csr/ClockCounterClockwise"
import { CloudSlashIcon } from "@phosphor-icons/react/dist/csr/CloudSlash"
import { ThermometerSimpleIcon } from "@phosphor-icons/react/dist/csr/ThermometerSimple"
import { WindIcon } from "@phosphor-icons/react/dist/csr/Wind"
import { DaySky } from "./weather-sky"
import { WeatherFailure } from "./weather-now"
import {
  degrees,
  Measure,
  percent,
  value,
  WeatherGlyph,
  weekdayName,
  type Days,
} from "./weather-figures"
import { weatherDescription, type WeatherRead } from "@/lib/weather"
import { cn } from "@/lib/utils"

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

/** Highs and lows on one shared scale, so the week reads as a shape. */
function weekScale(days: Days): (celsius: number) => number {
  const values = days.flatMap((day) =>
    [day.low_c, day.high_c].filter((amount) => amount !== null)
  )

  const low = Math.min(...values)
  const high = Math.max(...values)

  return (celsius) => (high === low ? 0.5 : (celsius - low) / (high - low))
}

/** Every hour's values for assistive technology; the sky beside it draws them. */
function HourlyForecast({
  hours,
}: {
  hours: Extract<WeatherRead, { state: "fresh" }>["forecast"]["hourly"]
}): React.JSX.Element {
  return (
    <section
      className="flex min-h-72 tablet:col-start-2 tablet:row-span-3 tablet:row-start-1"
      aria-labelledby="hourly-heading"
    >
      <div className="sr-only">
        <h2 id="hourly-heading">Next 24 hours</h2>
        <p>Walking or running?</p>
      </div>
      {hours.length === 0 ? (
        <p className="flex text-danger" role="status">
          <CloudSlashIcon size={28} weight="light" aria-hidden="true" />
          <span className="sr-only">Hourly forecast unavailable</span>
        </p>
      ) : (
        <>
          <DaySky hours={hours} />
          <div className="sr-only">
            <table>
              <caption>
                Temperature and wind at the start of each hour. Precipitation
                amount and chance cover the following hour, including rain,
                showers and snow. Missing values are unavailable.
              </caption>
              <thead>
                <tr>
                  <th scope="col">Hour starting</th>
                  <th scope="col">Temperature</th>
                  <th scope="col">Precipitation</th>
                  <th scope="col">Chance of precipitation</th>
                  <th scope="col">Wind</th>
                </tr>
              </thead>
              <tbody>
                {hours.map((hour) => (
                  <tr key={hour.starts_at}>
                    <th scope="row">
                      {date.format(new Date(hour.starts_at))}{" "}
                      <time dateTime={hour.starts_at}>
                        {hourTime.format(new Date(hour.starts_at))}
                      </time>{" "}
                      {
                        hourOffset
                          .formatToParts(new Date(hour.starts_at))
                          .find((part) => part.type === "timeZoneName")?.value
                      }
                      {hour.temperature_c === null &&
                      hour.wind_ms === null &&
                      hour.precipitation_mm === null &&
                      hour.precipitation_probability_percent === null
                        ? " Hourly conditions unavailable"
                        : null}
                    </th>
                    <td>{value(hour.temperature_c, "°C")}</td>
                    <td>{value(hour.precipitation_mm, "mm")}</td>
                    <td>
                      {value(hour.precipitation_probability_percent, "%")}
                    </td>
                    <td>{value(hour.wind_ms, "m/s")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </section>
  )
}

/** Whether the forecast still holds: nothing to see while fresh, a rewind mark when stale. The
 * page refreshes itself, so there is no manual check. */
function WeatherFreshness({
  weather,
}: {
  weather: Extract<WeatherRead, { state: "fresh" | "stale" }>
}): React.JSX.Element {
  const stale = weather.state === "stale"
  const fetched = new Date(weather.forecast.fetched_at)

  return (
    <div
      className={cn(
        "col-span-full flex items-center gap-3",
        !stale && "hidden"
      )}
      role="status"
    >
      {stale ? (
        <span
          className="inline-flex items-center gap-1.5 text-control font-semibold text-danger tabular-nums"
          aria-hidden="true"
        >
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
    </div>
  )
}

const attributionLink = "text-ink underline underline-offset-3"

const measure = "text-lead font-medium tracking-title tabular-nums"

const dayFigure = "[&>dd]:whitespace-nowrap [&>dd]:tabular-nums"

/**
 * The numbers on the left (now, then the week as ranges on one scale), the next day as a
 * horizon filling the right.
 */
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
    <div
      data-slot="wx"
      className="grid grid-cols-1 gap-x-pane-gap gap-y-7 *:min-w-0 tablet:min-h-fit-screen tablet:grid-cols-figures-fill tablet:grid-rows-hug-fill-hug"
      data-stale={weather.state === "stale"}
    >
      {weather.state === "stale" ? (
        <p className="sr-only">
          Showing the last cached forecast; conditions may have changed.
        </p>
      ) : null}
      <section
        className="grid grid-cols-hug-fill items-center gap-x-6 gap-y-5 tablet:col-start-1 tablet:row-start-1"
        aria-labelledby="current-heading"
      >
        <h2 id="current-heading" className="sr-only">
          Current conditions
        </h2>
        <p className="-ml-title-hang text-figure-xl leading-figure-xl font-medium tracking-figure-xl whitespace-nowrap tabular-nums in-data-[stale=true]:opacity-45">
          <span aria-hidden="true">{degrees(current?.temperature_c)}</span>
          <span className="sr-only">{value(current?.temperature_c, "°C")}</span>
        </p>
        <p className="flex justify-end in-data-[stale=true]:opacity-45">
          <WeatherGlyph
            code={current?.weather_code ?? null}
            size={96}
            className="h-auto w-condition-glyph"
          />
          <span className="sr-only">
            {weatherDescription(current?.weather_code ?? null)}
          </span>
        </p>
        <dl className="col-span-full flex gap-8 *:flex *:items-center *:gap-2.5">
          <div>
            <dt className="flex text-muted-ink">
              <ThermometerSimpleIcon size={22} aria-hidden="true" />
              <span className="sr-only">Feels like</span>
            </dt>
            <dd className={measure}>
              <span aria-hidden="true">{degrees(current?.feels_like_c)}</span>
              <span className="sr-only">
                {value(current?.feels_like_c, "°C")}
              </span>
            </dd>
          </div>
          <div>
            <dt className="flex text-muted-ink">
              <WindIcon size={22} aria-hidden="true" />
              <span className="sr-only">Wind</span>
            </dt>
            <dd className={measure}>
              <Measure amount={current?.wind_ms} unit="m/s" />
            </dd>
          </div>
        </dl>
        <WeatherFreshness weather={weather} />
        <p className="sr-only">
          {current === null
            ? "Current conditions unavailable"
            : `Modelled at ${timestamp.format(new Date(current.observed_at))}`}
        </p>
      </section>
      <HourlyForecast hours={forecast.hourly} />
      <section
        className="flex tablet:col-start-1 tablet:row-start-2"
        aria-labelledby="forecast-heading"
      >
        <h2 id="forecast-heading" className="sr-only">
          Seven days
        </h2>
        <ol className="flex flex-1 flex-col">
          {forecast.days.map((day, index) => (
            <li
              key={day.date}
              className="grid min-h-10 flex-1 grid-cols-week-row items-center gap-3"
              data-today={index === 0}
            >
              <h3 className="font-medium text-muted-ink in-data-[today=true]:text-ink">
                <time dateTime={day.date}>
                  <span aria-hidden="true">
                    {index === 0 ? "Today" : weekdayName(day.date)}
                  </span>
                  <span className="sr-only">
                    {date.format(new Date(`${day.date}T12:00:00Z`))}
                  </span>
                </time>
              </h3>
              <p className="flex">
                <WeatherGlyph code={day.weather_code} size={26} />
                <span className="sr-only">
                  {weatherDescription(day.weather_code)}
                </span>
              </p>
              <dl className="contents">
                <div
                  className={cn(
                    dayFigure,
                    "data-[gap=true]:gap-dash-4 [&>dd]:text-right [&>dd]:text-muted-ink"
                  )}
                  data-gap={day.low_c === null}
                >
                  <dt className="sr-only">Low</dt>
                  <dd>
                    <span aria-hidden="true">{degrees(day.low_c)}</span>
                    <span className="sr-only">{value(day.low_c, "°C")}</span>
                  </dd>
                </div>
                <div
                  className="relative h-1.5 rounded-full bg-wash-strong"
                  aria-hidden="true"
                >
                  {day.low_c === null || day.high_c === null ? null : (
                    <span
                      className="absolute inset-y-0 right-(--r) left-(--x) rounded-full bg-ink in-data-[today=true]:bg-sun"
                      // SAFETY: CSSProperties omits custom properties; --x and --r are the bar's start and remaining share of the week's temperature scale.
                      style={
                        {
                          "--x": percent(scale(day.low_c)),
                          "--r": percent(1 - scale(day.high_c)),
                        } as React.CSSProperties
                      }
                    />
                  )}
                </div>
                <div
                  className={cn(
                    dayFigure,
                    "data-[gap=true]:gap-dash-4 [&>dd]:font-semibold"
                  )}
                  data-gap={day.high_c === null}
                >
                  <dt className="sr-only">High</dt>
                  <dd>
                    <span aria-hidden="true">{degrees(day.high_c)}</span>
                    <span className="sr-only">{value(day.high_c, "°C")}</span>
                  </dd>
                </div>
                <div
                  className={cn(
                    dayFigure,
                    "data-[dry=true]:invisible data-[gap=true]:gap-dash-4 [&>dd]:text-right [&>dd]:text-sm [&>dd]:leading-normal [&>dd]:font-medium [&>dd]:text-rain"
                  )}
                  data-dry={day.precipitation_mm === 0}
                  data-gap={day.precipitation_mm === null}
                >
                  <dt className="sr-only">Precipitation</dt>
                  <dd>{value(day.precipitation_mm, "mm")}</dd>
                </div>
                <div
                  className="sr-only data-[gap=true]:gap-dash-4"
                  data-gap={day.wind_ms === null}
                >
                  <dt>Max wind</dt>
                  <dd>{value(day.wind_ms, "m/s")}</dd>
                </div>
              </dl>
            </li>
          ))}
        </ol>
      </section>
      <p className="text-label text-muted-ink tablet:col-start-1 tablet:row-start-3">
        Weather data by{" "}
        <a
          className={attributionLink}
          href="https://open-meteo.com/"
          target="_blank"
          rel="noreferrer"
        >
          Open-Meteo
        </a>
        , licensed under{" "}
        <a
          className={attributionLink}
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
