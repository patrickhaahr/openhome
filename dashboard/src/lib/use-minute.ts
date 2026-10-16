import { useEffect, useState } from "react"

/**
 * The display's one clock: hydrates from the rendered instant, then ticks on minute
 * boundaries while the page is visible and catches up when it becomes visible again.
 */
export function useMinute(initialTime: string): Date {
  const [now, setNow] = useState(() => new Date(initialTime))

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined

    function refresh(): void {
      clearTimeout(timer)

      if (document.visibilityState === "hidden") return

      setNow(new Date())
      timer = setTimeout(refresh, 60_000 - (Date.now() % 60_000))
    }

    refresh()
    document.addEventListener("visibilitychange", refresh)

    return () => {
      clearTimeout(timer)
      document.removeEventListener("visibilitychange", refresh)
    }
  }, [])

  return now
}
