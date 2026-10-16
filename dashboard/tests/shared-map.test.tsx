// @vitest-environment happy-dom

import { act } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { SharedMap } from "@/components/shared-map"

/**
 * Grant WebGL2 to the renderer probe's detached canvas only, so MapLibre is
 * chosen and then finds no context on its own attached canvas, as happy-dom has none.
 *
 * @returns Restores happy-dom's own `getContext`.
 */
function stubWebGL2Probe(): () => void {
  const original = Object.getOwnPropertyDescriptor(
    HTMLCanvasElement.prototype,
    "getContext"
  )

  // The probe only reads `getExtension` from the context it creates.
  Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
    configurable: true,
    value(this: HTMLCanvasElement, kind: string) {
      return kind === "webgl2" && !this.isConnected
        ? { getExtension: () => null }
        : null
    },
  })

  return () => {
    if (original !== undefined)
      Object.defineProperty(HTMLCanvasElement.prototype, "getContext", original)
  }
}

test("a loading timeout cannot be replaced by a late map import", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()
  const restoreProbe = stubWebGL2Probe()
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
    restoreProbe()
    vi.useRealTimers()
    vi.unstubAllGlobals()
  }
})

test("defers a hidden map, reports a MapLibre graphics failure and retries on request", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const restoreProbe = stubWebGL2Probe()
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
    restoreProbe()
    vi.unstubAllGlobals()
  }
})

test("without WebGL2 draws OpenStreetMap image tiles instead of loading MapLibre", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)

  try {
    await act(async () => {
      root.render(
        <SharedMap
          origin={{ latitude: 55.6869, longitude: 12.5692, source: "demo" }}
          routes={{
            lines: [
              {
                type: "LineString",
                coordinates: [
                  [12.5692, 55.6869],
                  [12.58, 55.69],
                ],
              },
            ],
            focused: 0,
          }}
        />
      )
    })

    await act(async () => {
      await import("leaflet")
    })

    const tile = await vi.waitFor(() => {
      const image =
        container.querySelector<HTMLImageElement>("img.leaflet-tile")

      if (image === null) throw new Error("Expected a map tile")

      return image
    })

    expect(tile.src).toMatch(
      /^https:\/\/tile\.openstreetmap\.org\/\d+\/\d+\/\d+\.png$/
    )
    expect(container.textContent).toContain("Loading map…")
    expect(container.querySelector(".maplibregl-canvas")).toBeNull()

    await act(async () => {
      tile.dispatchEvent(new Event("load"))
    })
    expect(container.textContent).not.toContain("Loading map…")
    expect(container.textContent).toContain("OpenStreetMap contributors")
    expect(container.querySelector('[aria-label="Zoom in"]')).not.toBeNull()
    expect(container.querySelector('[aria-label="3D buildings"]')).toBeNull()
    expect(container.querySelectorAll("svg path")).not.toHaveLength(0)
  } finally {
    await act(async () => {
      root.unmount()
    })
    container.remove()
    vi.unstubAllGlobals()
  }
})
