import { useEffect, useState } from "react"
import { unavailableWeather, type WeatherRead } from "@/lib/weather"

/**
 * The effect's single polling owner. Exactly one variant holds at a time, and only `enter`
 * replaces it, clearing a scheduled timer and never leaving `disposed`:
 *
 * - `scheduled`: a timer will refresh at validity or the retry interval.
 * - `reading`: one read is in flight. Hiding leaves it running (no abort signal exists), showing
 *   never starts a second read, and its settlement picks the next variant.
 * - `hidden`: off-screen with no timer and no read; showing refreshes.
 * - `disposed`: the effect ended; a late settlement is ignored.
 *
 * Fresh/stale/unavailable (`latest`) is the forecast's own fact and expires independently.
 * Axum owns cache policy; the browser only follows its `valid_until` and retries once a minute.
 *
 * The server hop to Axum is bounded (7s), but the browser's call to the dashboard server
 * function has no timeout, so a never-settling read keeps `reading` until disposal or the
 * effect re-running. A replaced effect cannot cancel its read, so one stale read may briefly
 * overlap the new effect's first read; its result is discarded.
 */
type Polling =
  | { state: "scheduled"; timer: ReturnType<typeof setTimeout> }
  | { state: "reading" }
  | { state: "hidden" }
  | { state: "disposed" }

const retryMs = 60_000

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

    // Leaves a settled or idle state: pause off-screen, else wait for validity or the retry.
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
        // A read in flight must stay `reading` so showing again cannot overlap it.
        if (!isReading()) enter({ state: "hidden" })

        return
      }

      void refresh()
    }

    // An already-expired initial forecast gets a zero delay and refreshes at once.
    park()
    document.addEventListener("visibilitychange", visibilityChanged)

    return () => {
      enter({ state: "disposed" })
      document.removeEventListener("visibilitychange", visibilityChanged)
    }
  }, [initial, readWeather])

  return children(snapshot.weather)
}
