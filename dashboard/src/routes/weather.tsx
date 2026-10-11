import { createFileRoute } from "@tanstack/react-router"
import { WeatherOverview } from "@/components/weather-overview"
import { getWeather } from "@/lib/weather.functions"
import { unavailableWeather } from "@/lib/weather"

export const Route = createFileRoute("/weather")({
  head: () => ({ meta: [{ title: "Weather · OpenHome" }] }),
  loader: () => getWeather().catch(() => unavailableWeather),
  headers: () => ({ "Cache-Control": "no-store" }),
  component: WeatherPage,
})

function WeatherPage(): React.JSX.Element {
  return (
    <section className="focused-page weather-page">
      <header>
        <p className="page-eyebrow">OpenHome / Weather</p>
        <h1 id="page-heading" tabIndex={-1}>
          Weather
        </h1>
        <p className="feature-intro">
          A look outside. Current conditions and the week ahead.
        </p>
      </header>
      <WeatherOverview weather={Route.useLoaderData()} />
    </section>
  )
}
