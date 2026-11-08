// @vitest-environment happy-dom

import { act } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { LiveWeather } from "@/components/live-weather"
import {
  weatherRead as weatherReadSchema,
  type WeatherRead,
} from "@/lib/weather"
import fixture from "./fixtures/weather.json"

test("expires an open forecast, retries failure, pauses while hidden and recovers on resume", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()
  vi.setSystemTime(new Date("2026-10-11T10:14:30Z"))
  const initial = weatherReadSchema.parse(fixture)
  let reads = 0
  let resolve: ((weather: WeatherRead) => void) | undefined

  const readWeather = (): Promise<WeatherRead> => {
    reads += 1

    return new Promise((done) => {
      resolve = done
    })
  }

  const container = document.createElement("div")
  const root = createRoot(container)

  try {
    await act(async () => {
      root.render(
        <LiveWeather initial={initial} readWeather={readWeather}>
          {(weather) => weather.state}
        </LiveWeather>
      )
    })
    expect(container.textContent).toBe("fresh")
    expect(reads).toBe(0)

    // Resuming just before expiry can return Axum's still-fresh cached read.
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"))
    })
    expect(reads).toBe(1)
    await act(async () => {
      resolve?.(initial)
    })
    expect(container.textContent).toBe("fresh")

    await act(async () => {
      await vi.advanceTimersByTimeAsync(30_000)
    })
    expect(container.textContent).toBe("stale")
    expect(reads).toBe(2)
    await act(async () => {
      resolve?.({ state: "unavailable", reason: "provider-unavailable" })
    })
    expect(container.textContent).toBe("unavailable")
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    expect(reads).toBe(3)
    await act(async () => {
      resolve?.({ state: "unavailable", reason: "provider-unavailable" })
    })

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    })
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"))
      await vi.advanceTimersByTimeAsync(24 * 3_600_000)
    })
    expect(reads).toBe(3)

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    })
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"))
    })
    expect(reads).toBe(4)

    const recovered = weatherReadSchema.parse({
      ...fixture,
      forecast: {
        ...fixture.forecast,
        fetched_at: new Date().toISOString(),
        valid_until: new Date(Date.now() + 900_000).toISOString(),
      },
    })

    await act(async () => {
      resolve?.(recovered)
    })
    expect(container.textContent).toBe("fresh")
    await act(async () => {
      await vi.advanceTimersByTimeAsync(899_999)
    })
    expect(reads).toBe(4)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1)
    })
    expect(reads).toBe(5)
    expect(container.textContent).toBe("stale")
  } finally {
    await act(async () => {
      root.unmount()
    })
    Reflect.deleteProperty(document, "visibilityState")
    vi.useRealTimers()
    vi.unstubAllGlobals()
  }
})

test("keeps one read through hide/show, then ignores late reads after replacement and disposal", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()
  vi.setSystemTime(new Date("2026-10-11T10:14:30Z"))
  const initial = weatherReadSchema.parse(fixture)

  const unavailable: WeatherRead = {
    state: "unavailable",
    reason: "provider-unavailable",
  }

  let reads = 0
  let resolve: ((weather: WeatherRead) => void) | undefined

  const readWeather = (): Promise<WeatherRead> => {
    reads += 1

    return new Promise((done) => {
      resolve = done
    })
  }

  const show = (value: "hidden" | "visible") =>
    act(async () => {
      Object.defineProperty(document, "visibilityState", {
        configurable: true,
        value,
      })
      document.dispatchEvent(new Event("visibilitychange"))
    })

  const view = (weather: WeatherRead) => (
    <LiveWeather initial={weather} readWeather={readWeather}>
      {(read) => read.state}
    </LiveWeather>
  )

  const container = document.createElement("div")
  const root = createRoot(container)

  try {
    await act(async () => {
      root.render(view(initial))
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(30_000)
    })
    expect(reads).toBe(1)

    // Hiding and showing during a read never starts a second one.
    await show("hidden")
    await show("visible")
    expect(reads).toBe(1)

    // A read that settles while hidden applies its result but schedules nothing.
    await show("hidden")
    await act(async () => {
      resolve?.(unavailable)
    })
    expect(container.textContent).toBe("unavailable")
    await act(async () => {
      await vi.advanceTimersByTimeAsync(24 * 3_600_000)
    })
    expect(reads).toBe(1)

    await show("visible")
    expect(reads).toBe(2)
    await act(async () => {
      resolve?.(unavailable)
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    expect(reads).toBe(3)

    // A new forecast replaces the effect while a read is in flight: the old
    // result is discarded and polling follows the new validity.
    const replacement = weatherReadSchema.parse({
      ...fixture,
      forecast: {
        ...fixture.forecast,
        fetched_at: new Date().toISOString(),
        valid_until: new Date(Date.now() + 900_000).toISOString(),
      },
    })

    await act(async () => {
      root.render(view(replacement))
    })
    expect(container.textContent).toBe("fresh")
    await act(async () => {
      resolve?.(unavailable)
    })
    expect(container.textContent).toBe("fresh")
    await act(async () => {
      await vi.advanceTimersByTimeAsync(899_999)
    })
    expect(reads).toBe(3)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1)
    })
    expect(reads).toBe(4)

    // Disposal during a read ignores its result and removes the listener.
    await act(async () => {
      root.unmount()
    })
    await act(async () => {
      resolve?.(replacement)
    })
    document.dispatchEvent(new Event("visibilitychange"))
    await vi.advanceTimersByTimeAsync(24 * 3_600_000)
    expect(reads).toBe(4)
    expect(container.textContent).toBe("")
  } finally {
    Reflect.deleteProperty(document, "visibilityState")
    vi.useRealTimers()
    vi.unstubAllGlobals()
  }
})
