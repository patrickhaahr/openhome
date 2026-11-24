// @vitest-environment happy-dom

import { act, createElement } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { LiveClock } from "@/components/live-clock"

const COPENHAGEN_HALF_MINUTE_BEFORE_MIDNIGHT = "2026-10-11T21:59:30.000Z"

const HALF_MINUTE_MS = 30_000

const HOUR_MS = 3_600_000

test("rolls the rendered clock and date over midnight and catches up when the display becomes visible", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()
  vi.setSystemTime(new Date(COPENHAGEN_HALF_MINUTE_BEFORE_MIDNIGHT))
  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)

  try {
    await act(async () => {
      root.render(
        createElement(LiveClock, {
          initialTime: COPENHAGEN_HALF_MINUTE_BEFORE_MIDNIGHT,
        })
      )
    })
    expect(container.querySelector("time")?.textContent).toBe("23:59")
    expect(container.textContent).toContain("Sunday, 11 October 2026")

    await act(async () => {
      await vi.advanceTimersByTimeAsync(HALF_MINUTE_MS)
    })
    expect(container.querySelector("time")?.textContent).toBe("00:00")
    expect(container.textContent).toContain("Monday, 12 October 2026")

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    })
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"))
      await vi.advanceTimersByTimeAsync(HOUR_MS)
    })
    expect(container.querySelector("time")?.textContent).toBe("00:00")

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    })
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"))
    })
    expect(container.querySelector("time")?.textContent).toBe("01:00")
  } finally {
    await act(async () => {
      root.unmount()
    })
    container.remove()
    Reflect.deleteProperty(document, "visibilityState")
    vi.useRealTimers()
    vi.unstubAllGlobals()
  }
})
