import { useEffect, useState } from "react"
import { unavailableWeather, type WeatherRead } from "@/lib/weather"

type Polling =
  | { state: "scheduled"; timer: ReturnType<typeof setTimeout> }
  | { state: "reading" }
  | { state: "hidden" }
  | { state: "disposed" }

const retryMs = 60_000

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
    let polling: Polling = { state: "hidden" }

    function enter(next: Polling): void {
      if (polling.state === "disposed") {
        if (next.state === "scheduled") clearTimeout(next.timer)

        return
      }

      if (polling.state === "scheduled") clearTimeout(polling.timer)
      polling = next
    }

    function isReading(): boolean {
      return polling.state === "reading"
    }

    function isDisposed(): boolean {
      return polling.state === "disposed"
    }

    function isHidden(): boolean {
      return document.visibilityState === "hidden"
    }

    function expire(): void {
      if (
        latest.state === "fresh" &&
        Date.parse(latest.forecast.valid_until) <= Date.now()
      ) {
        latest = { state: "stale", forecast: latest.forecast }
      }

      setSnapshot({ initial, weather: latest })
    }

    function park(): void {
      if (isHidden()) {
        enter({ state: "hidden" })

        return
      }

      const delay =
        latest.state === "fresh"
          ? Date.parse(latest.forecast.valid_until) - Date.now()
          : retryMs

      enter({
        state: "scheduled",
        timer: setTimeout(() => {
          void refresh()
        }, delay),
      })
    }

    async function refresh(): Promise<void> {
      if (isDisposed()) return

      expire()

      if (isReading()) return
      enter({ state: "reading" })

      const result = await readWeather().catch(() => unavailableWeather)

      if (isDisposed()) return

      latest = result
      expire()
      park()
    }

    function visibilityChanged(): void {
      if (isHidden()) {
        if (!isReading()) enter({ state: "hidden" })

        return
      }

      void refresh()
    }

    park()
    document.addEventListener("visibilitychange", visibilityChanged)

    return () => {
      enter({ state: "disposed" })
      document.removeEventListener("visibilitychange", visibilityChanged)
    }
  }, [initial, readWeather])

  return children(snapshot.weather)
}
