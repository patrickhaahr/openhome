import { useEffect, useState } from "react"
import { copenhagenDay } from "./important-dates"

/** The Copenhagen day, changing at its actual midnight even across daylight-saving changes. */
export function useCopenhagenDay(): string {
  const [day, setDay] = useState(() => copenhagenDay(new Date()))

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>

    function schedule(): void {
      clearTimeout(timer)
      const now = Date.now()
      const today = copenhagenDay(new Date(now))
      setDay(today)
      // Find the first instant of the next Copenhagen day; it can be 23, 24 or 25 hours away.
      let low = now
      let high = now + 27 * 60 * 60 * 1000

      while (high - low > 1) {
        const middle = Math.floor((low + high) / 2)

        if (copenhagenDay(new Date(middle)) === today) low = middle
        else high = middle
      }

      timer = setTimeout(schedule, high - now)
    }

    schedule()
    document.addEventListener("visibilitychange", schedule)

    return () => {
      clearTimeout(timer)
      document.removeEventListener("visibilitychange", schedule)
    }
  }, [])

  return day
}
