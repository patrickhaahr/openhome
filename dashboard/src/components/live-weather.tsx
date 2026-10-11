import { useEffect, useState } from "react"
import { unavailableWeather, type WeatherRead } from "@/lib/weather"

/** Refresh only weather, following Axum validity and pausing off-screen. */
export function LiveWeather({
  initial,
  readWeather,
  children,
}: {
  initial: WeatherRead
  readWeather: () => Promise<WeatherRead>
  children: (weather: WeatherRead) => React.ReactNode
}): React.ReactNode {
  const [snapshot, setSnapshot] = useState({ initial, weather: initial })

  if (snapshot.initial !== initial) {
    setSnapshot({ initial, weather: initial })
  }

  useEffect(() => {
    let latest = initial
    let pending = false
    let disposed = false
    let timer: ReturnType<typeof setTimeout> | undefined

    function expire(): void {
      if (
        latest.state === "fresh" &&
        Date.parse(latest.forecast.valid_until) <= Date.now()
      ) {
        latest = { state: "stale", forecast: latest.forecast }
      }

      setSnapshot({ initial, weather: latest })
    }

    function schedule(): void {
      clearTimeout(timer)

      if (document.visibilityState === "hidden") return

      const delay =
        latest.state === "fresh"
          ? Date.parse(latest.forecast.valid_until) - Date.now()
          : 60_000

      timer = setTimeout(() => {
        void refresh()
      }, delay)
    }

    async function refresh(): Promise<void> {
      clearTimeout(timer)

      if (document.visibilityState === "hidden") return
      expire()

      if (pending) return
      pending = true

      const read = await readWeather().catch(() => unavailableWeather)

      if (disposed) return

      pending = false
      latest = read
      expire()

      schedule()
    }

    function visibilityChanged(): void {
      clearTimeout(timer)

      if (document.visibilityState !== "hidden") void refresh()
    }

    if (
      initial.state === "fresh" &&
      Date.parse(initial.forecast.valid_until) <= Date.now()
    ) {
      timer = setTimeout(() => {
        void refresh()
      }, 0)
    } else {
      schedule()
    }

    document.addEventListener("visibilitychange", visibilityChanged)

    return () => {
      disposed = true
      clearTimeout(timer)
      document.removeEventListener("visibilitychange", visibilityChanged)
    }
  }, [initial, readWeather])

  return children(snapshot.weather)
}
