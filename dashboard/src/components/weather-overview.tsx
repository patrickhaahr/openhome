import { Suspense } from "react"
import { Await, Link } from "@tanstack/react-router"
import { ArrowRightIcon } from "@phosphor-icons/react/dist/csr/ArrowRight"
import { CloudSunIcon } from "@phosphor-icons/react/dist/csr/CloudSun"
import { CheckAgainButton } from "./check-again-button"
import { LiveWeather } from "./live-weather"
import { Card, CardContent, CardHeader } from "./ui/card"
import { weatherDescription, type WeatherRead } from "@/lib/weather"
import { getWeather } from "@/lib/weather.functions"

const number = new Intl.NumberFormat("da-DK", { maximumFractionDigits: 1 })

const timestamp = new Intl.DateTimeFormat("da-DK", {
  timeZone: "Europe/Copenhagen",
  day: "numeric",
  month: "short",
  hour: "2-digit",
  minute: "2-digit",
})

const date = new Intl.DateTimeFormat("da-DK", {
  timeZone: "Europe/Copenhagen",
  weekday: "short",
  day: "numeric",
  month: "short",
})

const hourTime = new Intl.DateTimeFormat("da-DK", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
})

const hourOffset = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  timeZoneName: "shortOffset",
})

function value(amount: number | null | undefined, unit: string): string {
  return amount === null || amount === undefined
    ? "Unavailable"
    : `${number.format(amount)} ${unit}`
}

function WeatherFailure({
  reason,
}: {
  reason: "missing-configuration" | "provider-unavailable"
}): React.JSX.Element {
  return (
    <div className="weather-empty" role="status">
      <h3>
        {reason === "missing-configuration"
          ? "Weather setup needed"
          : "Weather unavailable"}
      </h3>
      <p>
        {reason === "missing-configuration"
          ? "Set the private forecast location on OpenHome to see local conditions."
          : "OpenHome could not provide a forecast. Check Connection or try again."}
      </p>
      <CheckAgainButton />
    </div>
  )
}

function HourlyForecast({
  hours,
}: {
  hours: Extract<WeatherRead, { state: "fresh" }>["forecast"]["hourly"]
}): React.JSX.Element {
  const peak = Math.max(1, ...hours.map((hour) => hour.precipitation_mm ?? 0))

  return (
    <section className="weather-hourly" aria-labelledby="hourly-heading">
      <div className="weather-week-heading">
        <div>
          <p className="weather-outdoor-label">Walking or running?</p>
          <h2 id="hourly-heading">Next 24 hours</h2>
        </div>
        <p>Copenhagen time · Next hours highlighted</p>
      </div>
      {hours.length === 0 ? (
        <p className="weather-empty">Hourly forecast unavailable</p>
      ) : (
        <>
          <p id="hourly-help" className="weather-meta">
            Compare temperature, precipitation and wind before heading out.
            Swipe or scroll across for later hours. Missing values are
            unavailable.
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
                  <th scope="col">Hour starting</th>
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
                  <th scope="row">Temperature</th>
                  {hours.map((hour) => (
                    <td key={hour.starts_at}>
                      {value(hour.temperature_c, "°C")}
                    </td>
                  ))}
                </tr>
                <tr>
                  <th scope="row">
                    Precipitation
                    <span className="weather-hour-note">Over this hour</span>
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
                      {value(hour.precipitation_mm, "mm")}
                    </td>
                  ))}
                </tr>
                <tr>
                  <th scope="row">
                    Chance
                    <span className="weather-hour-note">Of precipitation</span>
                  </th>
                  {hours.map((hour) => (
                    <td key={hour.starts_at}>
                      {value(hour.precipitation_probability_percent, "%")}
                    </td>
                  ))}
                </tr>
                <tr>
                  <th scope="row">Wind</th>
                  {hours.map((hour) => (
                    <td key={hour.starts_at}>{value(hour.wind_ms, "m/s")}</td>
                  ))}
                </tr>
              </tbody>
            </table>
          </div>
          <p className="weather-meta">
            Precipitation includes rain, showers and snow. Each amount and
            chance covers the hour starting above; temperature and wind are at
            its start.
          </p>
        </>
      )}
    </section>
  )
}

/** Glanceable Home weather, deferred independently of home controls. */
export function WeatherCard({
  weather,
}: {
  weather: Promise<WeatherRead>
}): React.JSX.Element {
  return (
    <div className="weather-card">
      <Card className="h-full">
        <CardHeader>
          <div className="feature-card-top">
            <CloudSunIcon
              size={25}
              aria-hidden="true"
              className="text-muted-foreground"
            />
          </div>
          <h2 className="feature-card-title">Weather</h2>
        </CardHeader>
        <CardContent className="flex-1">
          <div className="flex h-full flex-col gap-2">
            <Suspense fallback={<p role="status">Checking weather…</p>}>
              <Await promise={weather}>
                {(initial) => (
                  <LiveWeather initial={initial} readWeather={getWeather}>
                    {(read) =>
                      read.state === "unavailable" ? (
                        <WeatherFailure reason={read.reason} />
                      ) : (
                        <>
                          <p className="weather-card-temperature">
                            {value(read.forecast.current?.temperature_c, "°C")}
                          </p>
                          <p>
                            {weatherDescription(
                              read.forecast.current?.weather_code ?? null
                            )}
                          </p>
                          <p className="weather-meta">
                            {read.state === "stale"
                              ? "Stale forecast · "
                              : "Fetched · "}
                            {timestamp.format(
                              new Date(read.forecast.fetched_at)
                            )}
                          </p>
                          <p className="weather-meta">Copenhagen time</p>
                        </>
                      )
                    }
                  </LiveWeather>
                )}
              </Await>
            </Suspense>
            <Link
              to="/weather"
              aria-label="Open Weather"
              className="feature-card-link"
            >
              Hourly &amp; seven-day forecast{" "}
              <ArrowRightIcon size={18} aria-hidden="true" />
            </Link>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

/** Current conditions and seven days from the same cached Axum forecast. */
export function WeatherOverview({
  weather,
}: {
  weather: WeatherRead
}): React.JSX.Element {
  if (weather.state === "unavailable")
    return (
      <Card>
        <CardContent>
          <WeatherFailure reason={weather.reason} />
        </CardContent>
      </Card>
    )

  const { forecast } = weather
  const current = forecast.current

  return (
    <>
      <div className="weather-freshness" role="status">
        <span className="weather-state" data-stale={weather.state === "stale"}>
          {weather.state === "stale" ? "Stale forecast" : "Fresh forecast"}
        </span>
        <span>
          Fetched {timestamp.format(new Date(forecast.fetched_at))} · Valid
          until {timestamp.format(new Date(forecast.valid_until))}
        </span>
        <CheckAgainButton />
      </div>
      {weather.state === "stale" ? (
        <p className="weather-stale-note">
          Showing the last cached forecast; conditions may have changed.
        </p>
      ) : null}
      <div className="weather-current">
        <Card>
          <CardHeader>
            <h2>Current conditions</h2>
            <p className="weather-meta">
              {current === null
                ? "Current conditions unavailable"
                : `Modelled at ${timestamp.format(new Date(current.observed_at))}`}
            </p>
          </CardHeader>
          <CardContent>
            <div className="weather-current-body">
              <div>
                <p className="weather-temperature">
                  {value(current?.temperature_c, "°C")}
                </p>
                <p className="weather-condition">
                  {weatherDescription(current?.weather_code ?? null)}
                </p>
              </div>
              <dl className="weather-measures">
                <div>
                  <dt>Feels like</dt>
                  <dd>{value(current?.feels_like_c, "°C")}</dd>
                </div>
                <div>
                  <dt>Wind</dt>
                  <dd>{value(current?.wind_ms, "m/s")}</dd>
                </div>
              </dl>
            </div>
          </CardContent>
        </Card>
      </div>
      <HourlyForecast hours={forecast.hourly} />
      <section aria-labelledby="forecast-heading">
        <div className="weather-week-heading">
          <h2 id="forecast-heading">Seven-day forecast</h2>
          <p>Copenhagen time · °C · mm · m/s</p>
        </div>
        <div className="weather-days">
          {forecast.days.map((day) => (
            <article key={day.date} className="weather-day">
              <Card className="h-full">
                <CardHeader>
                  <h3>
                    <time dateTime={day.date}>
                      {date.format(new Date(`${day.date}T12:00:00Z`))}
                    </time>
                  </h3>
                  <p className="weather-day-condition">
                    {weatherDescription(day.weather_code)}
                  </p>
                </CardHeader>
                <CardContent>
                  <dl>
                    <div className="weather-day-temperature">
                      <dt>High</dt>
                      <dd>{value(day.high_c, "°C")}</dd>
                    </div>
                    <div>
                      <dt>Low</dt>
                      <dd>{value(day.low_c, "°C")}</dd>
                    </div>
                    <div>
                      <dt>Precipitation</dt>
                      <dd>{value(day.precipitation_mm, "mm")}</dd>
                    </div>
                    <div>
                      <dt>Max wind</dt>
                      <dd>{value(day.wind_ms, "m/s")}</dd>
                    </div>
                  </dl>
                </CardContent>
              </Card>
            </article>
          ))}
        </div>
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
        . Forecasts can change.
      </p>
    </>
  )
}
