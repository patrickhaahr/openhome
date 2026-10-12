// @vitest-environment happy-dom

import { act } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { SharedMap } from "@/components/shared-map"

test("a loading timeout cannot be replaced by a late map import", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()
  const container = document.createElement("div")
  const root = createRoot(container)

  try {
    act(() => {
      root.render(
        <SharedMap
          origin={{ latitude: 55.6869, longitude: 12.5692, source: "demo" }}
        />
      )
    })
    act(() => {
      vi.advanceTimersByTime(20_000)
    })
    expect(container.textContent).toContain("Map tiles unavailable")

    await act(async () => {
      await import("maplibre-gl")
    })
    expect(container.textContent).toContain("Map tiles unavailable")
    expect(container.querySelector("canvas")).toBeNull()
  } finally {
    await act(async () => root.unmount())
    vi.useRealTimers()
    vi.unstubAllGlobals()
  }
})

test("defers a hidden map, falls back when WebGL2 is unavailable and retries on request", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const visibility = vi.spyOn(document, "hidden", "get").mockReturnValue(true)
  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)

  async function settled(): Promise<void> {
    await vi.waitFor(async () => {
      await act(async () => {
        await import("maplibre-gl")
      })
      expect(container.textContent).toContain("Map graphics unavailable")
    })
  }

  try {
    await act(async () => {
      root.render(
        <SharedMap
          origin={{ latitude: 55.6869, longitude: 12.5692, source: "demo" }}
        />
      )
      await import("maplibre-gl")
    })
    expect(container.textContent).toContain("Loading map…")

    visibility.mockReturnValue(false)
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"))
    })
    await settled()
    expect(container.textContent).toContain(
      "The rest of the dashboard still works."
    )
    expect(container.querySelector('[aria-label="Zoom in"]')).toBeNull()

    const retry = [...container.querySelectorAll("button")].find(
      (button) => button.textContent === "Try again"
    )

    if (retry === undefined) throw new Error("Expected a retry button")

    await act(async () => {
      retry.click()
    })
    await settled()
  } finally {
    await act(async () => {
      root.unmount()
    })
    container.remove()
    visibility.mockRestore()
    vi.unstubAllGlobals()
  }
})
