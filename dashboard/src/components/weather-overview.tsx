import { Suspense } from "react"
import { Await, Link } from "@tanstack/react-router"
import { ArrowRightIcon } from "@phosphor-icons/react/dist/csr/ArrowRight"
import { CloudSunIcon } from "@phosphor-icons/react/dist/csr/CloudSun"
import { CheckAgainButton } from "./check-again-button"
import { Card, CardContent, CardHeader } from "./ui/card"
import { weatherDescription, type WeatherRead } from "@/lib/weather"

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
                        {timestamp.format(new Date(read.forecast.fetched_at))}
                      </p>
                      <p className="weather-meta">Copenhagen time</p>
                    </>
                  )
                }
              </Await>
            </Suspense>
            <Link
              to="/weather"
              aria-label="Open Weather"
              className="feature-card-link"
            >
              Seven-day forecast <ArrowRightIcon size={18} aria-hidden="true" />
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
          Refresh unavailable. Showing the last cached forecast; conditions may
          have changed.
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
