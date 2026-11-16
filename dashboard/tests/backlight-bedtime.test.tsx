// @vitest-environment happy-dom
import { act } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { BacklightControls } from "@/components/backlight-controls"

test("Bedtime controls show the saved schedule and resume Automatic with override expiry", async () => {
  const { backlightStatus } = await import("@/lib/backlight")

  const status = backlightStatus.parse({
    setup: "ready",
    reachability: "reachable",
    intended: { mode: "off" },
    acknowledged: { mode: "off" },
    observed: null,
    stale: true,
    command: "acknowledged",
    bedtime: { enabled: true, start: "23:00", end: "00:00", brightness: 20 },
    control_source: "manual",
    override_expires_at: "2026-10-23T22:00:00Z",
    next_transition: "2026-10-23T22:00:00Z",
  })

  const actions: import("@/lib/backlight").BacklightAction[] = []
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)

  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)

  try {
    await act(async () =>
      root.render(
        <BacklightControls
          data={{ state: "available", status }}
          change={async (action) => {
            actions.push(action)

            return {
              state: "available",
              status: {
                ...status,
                control_source: "automatic",
                override_expires_at: null,
                intended: { mode: "tv-sync" },
              },
            }
          }}
        />
      )
    )
    expect(container.textContent).toContain("23:00–00:00")
    expect(container.textContent).toContain("Manual override")
    expect(container.querySelector("time")?.getAttribute("dateTime")).toBe(
      "2026-10-23T22:00:00Z"
    )

    const automatic = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "Automatic"
    )

    expect(automatic).toBeDefined()
    await act(async () => automatic?.click())
    expect(actions[0]).toEqual({ action: "automatic" })
    expect(container.textContent).not.toContain("Manual override")
    const enabled = container.querySelector('[role="switch"]')

    expect(enabled?.getAttribute("aria-checked")).toBe("true")
    await act(async () => {
      if (enabled instanceof HTMLElement) enabled.click()
    })
    expect(actions[1]).toEqual({ action: "bedtime", enabled: false })
  } finally {
    await act(async () => root.unmount())
    container.remove()
    vi.unstubAllGlobals()
  }
})

test("Bedtime dial never collapses the window, saves one settled edit, and springs back when refused", async () => {
  const { backlightStatus } = await import("@/lib/backlight")

  const status = backlightStatus.parse({
    setup: "ready",
    reachability: "reachable",
    intended: { mode: "off" },
    acknowledged: { mode: "off" },
    observed: null,
    stale: true,
    command: "acknowledged",
    bedtime: { enabled: false, start: "23:00", end: "00:00", brightness: 20 },
    control_source: "manual",
    override_expires_at: null,
    next_transition: null,
  })

  const actions: import("@/lib/backlight").BacklightAction[] = []
  let reject = false
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()

  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)

  async function press(label: string, ...keys: string[]): Promise<void> {
    for (const key of keys)
      await act(async () => {
        container.querySelector(`[aria-label="${label}"]`)?.dispatchEvent(
          new KeyboardEvent("keydown", {
            key,
            bubbles: true,
            cancelable: true,
          })
        )
      })
  }

  const shown = (label: string): string | null | undefined =>
    container
      .querySelector(`[aria-label="${label}"]`)
      ?.getAttribute("aria-valuetext")

  async function settle(): Promise<void> {
    await act(async () => vi.advanceTimersByTimeAsync(800))
  }

  try {
    await act(async () =>
      root.render(
        <BacklightControls
          data={{ state: "available", status }}
          change={async (action) => {
            actions.push(action)

            if (reject) return { state: "invalid-settings" }

            return {
              state: "available",
              status: {
                ...status,
                bedtime: {
                  enabled: false,
                  start: "22:30",
                  end: "02:00",
                  brightness: 37,
                },
                override_expires_at: "2026-10-24T00:00:00Z",
              },
            }
          }}
        />
      )
    )
    // Walking End back onto Start hops over it instead of emptying the window.
    await press("End", "ArrowDown", "ArrowDown", "ArrowDown", "ArrowDown")
    expect(shown("End")).toBe("22:45")
    await press("End", "PageUp", "PageUp", "PageUp", "ArrowUp")
    await press("Start", "ArrowLeft", "ArrowLeft")
    await press("Red brightness", "PageUp", "ArrowUp", "ArrowUp")
    await press("Red brightness", ...Array<string>(5).fill("ArrowUp"))
    expect(actions).toEqual([])
    await settle()
    expect(actions).toEqual([
      { action: "bedtime", start: "22:30", end: "02:00", brightness: 37 },
    ])
    expect(container.textContent).toContain("22:30–02:00")
    expect(container.textContent).toContain("Manual override")
    expect(container.querySelector("time")?.dateTime).toBe(
      "2026-10-24T00:00:00Z"
    )
    expect(container.querySelector('[role="status"]')?.textContent).toContain(
      "Bedtime settings saved"
    )
    expect(
      container
        .querySelector('[aria-label="Bedtime Mode enabled"]')
        ?.getAttribute("aria-checked")
    ).toBe("false")
    reject = true
    await press("Start", ...Array<string>(6).fill("ArrowLeft"))
    expect(shown("Start")).toBe("21:00")
    await settle()
    expect(actions.at(-1)).toEqual({
      action: "bedtime",
      start: "21:00",
      end: "02:00",
    })
    expect(container.querySelector('[role="status"]')?.textContent).toContain(
      "weren’t saved"
    )
    expect(container.textContent).toContain("Not saved")
    expect(shown("Start")).toBe("22:30")
  } finally {
    await act(async () => root.unmount())
    container.remove()
    vi.useRealTimers()
    vi.unstubAllGlobals()
  }
})
