import { useEffect, useState } from "react"
import { copenhagenDay } from "./important-dates"

export function useCopenhagenDay(): string {
  const [day, setDay] = useState(() => copenhagenDay(new Date()))

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>

    function schedule(): void {
      clearTimeout(timer)
      const now = Date.now()
      const today = copenhagenDay(new Date(now))
      setDay(today)
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
