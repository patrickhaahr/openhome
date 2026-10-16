import { createFileRoute } from "@tanstack/react-router"
import { PageHead } from "@/components/page-head"
import { WeatherOverview } from "@/components/weather-overview"
import { LiveWeather } from "@/components/live-weather"
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
    <LiveWeather initial={Route.useLoaderData()} readWeather={getWeather}>
      {(weather) => (
        <>
          <PageHead title="Weather" />
          <WeatherOverview weather={weather} />
        </>
      )}
    </LiveWeather>
  )
}
