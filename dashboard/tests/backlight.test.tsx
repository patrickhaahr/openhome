// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
import { act } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { BacklightControls } from "@/components/backlight-controls"
import { readBacklight, changeBacklight } from "@/lib/backlight.server"
import { startAxumFixture } from "./support/axum-fixture"

test("incomplete setup keeps Off available and announces failed hardware writes honestly", async () => {
  const fixture = await startAxumFixture()
  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)

  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)
  let work: Promise<unknown> = Promise.resolve()

  try {
    const data = await readBacklight()
    expect(await changeBacklight({ action: "automatic" })).toEqual({
      state: "profile-required",
    })
    expect(await changeBacklight({ action: "bedtime", enabled: true })).toEqual(
      {
        state: "profile-required",
      }
    )
    await act(async () =>
      root.render(
        <BacklightControls
          data={data}
          change={(action) => {
            const result = changeBacklight(action)
            work = result

            return result
          }}
        />
      )
    )
    expect(container.textContent).toContain("Save your TV Sync settings")

    const off = Array.from(container.querySelectorAll("button")).find(
      (b) => b.textContent === "Off"
    )

    expect(off?.disabled).toBe(false)
    await act(async () => {
      off?.click()
      await work
    })
    expect(container.querySelector("[role=status]")?.textContent).toContain(
      "couldn’t confirm"
    )
    expect(container.textContent).toContain("Unknown")

    const saved = await changeBacklight({
      action: "bedtime",
      start: "22:30",
      end: "02:00",
      brightness: 37,
    })

    expect(saved.state).toBe("available")

    const persisted = await readBacklight()
    expect(persisted.state === "available" && persisted.status.bedtime).toEqual(
      { enabled: false, start: "22:30", end: "02:00", brightness: 37 }
    )
  } finally {
    await act(async () => root.unmount())
    container.remove()
    vi.unstubAllGlobals()
    vi.unstubAllEnvs()
    await fixture.stop()
  }
})

test("RGB and brightness edits coalesce, keep the newest edit during dispatch, and expose requested rather than confirmed color", async () => {
  const { backlightStatus } = await import("@/lib/backlight")

  const initial = backlightStatus.parse({
    bedtime: { enabled: false, start: "23:00", end: "00:00", brightness: 20 },
    control_source: "manual",
    override_expires_at: null,
    next_transition: null,
    setup: "ready",
    reachability: "reachable",
    intended: { mode: "tv-sync" },
    acknowledged: { mode: "tv-sync" },
    stale: false,
    command: "idle",
    observed: {
      power: true,
      brightness: 45,
      mode: "tv-sync",
      observed_at: "2026-10-23T18:00:00Z",
    },
  })

  const accepted: import("@/lib/backlight").BacklightAction[] = []
  let finish: (() => void) | undefined
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()
  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)

  async function press(label: string, ...keys: string[]): Promise<void> {
    const control = container.querySelector(`[aria-label="${label}"]`)
    expect(control).toBeInstanceOf(Element)

    for (const key of keys)
      await act(async () => {
        control?.dispatchEvent(
          new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true })
        )
      })
  }

  try {
    await act(async () =>
      root.render(
        <BacklightControls
          data={{ state: "available", status: initial }}
          change={async (action) => {
            accepted.push(action)

            if (accepted.length === 1)
              await new Promise<void>((resolve) => {
                finish = resolve
              })

            return {
              state: "available",
              status: {
                ...initial,
                intended:
                  action.action === "select"
                    ? action.selection
                    : initial.intended,
                command: "acknowledged",
                acknowledged:
                  action.action === "select"
                    ? action.selection
                    : initial.acknowledged,
              },
            }
          }}
        />
      )
    )
    // Six hue steps of 5° from red land on orange; Home then two pages set 20 %.
    await press("RGB color", ...Array<string>(6).fill("ArrowRight"))
    await press("Brightness", "Home", "PageUp", "PageUp")
    expect(accepted).toEqual([])
    await act(async () => vi.advanceTimersByTimeAsync(250))
    expect(accepted).toEqual([
      {
        action: "select",
        selection: {
          mode: "color",
          rgb: { red: 255, green: 128, blue: 0 },
          brightness: 20,
        },
      },
    ])
    await press("RGB color", "PageUp", "PageUp", "PageUp")
    expect(
      container
        .querySelector('[aria-label="RGB color"]')
        ?.getAttribute("aria-valuetext")
    ).toBe("0, 255, 0")
    await press("RGB color", "PageUp", "PageUp", "PageUp", "PageUp")
    await press("Brightness", "PageUp")
    await act(async () => vi.advanceTimersByTimeAsync(250))
    expect(accepted).toHaveLength(1)
    expect(
      container.querySelector("[aria-busy]")?.getAttribute("aria-busy")
    ).toBe("true")
    await act(async () => finish?.())
    expect(accepted).toHaveLength(2)
    expect(accepted[1]).toEqual({
      action: "select",
      selection: {
        mode: "color",
        rgb: { red: 0, green: 0, blue: 255 },
        brightness: 30,
      },
    })
    expect(container.textContent).toContain("Requested RGB")
    expect(container.textContent).toContain("0, 0, 255")
    expect(container.textContent).toContain("Observed brightness")
    expect(container.textContent).toContain("45%")
    expect(container.textContent).toContain("does not confirm")
    expect(container.textContent).toContain("until you choose again")
    await press("Brightness", "PageUp", "PageUp", "PageUp")
    await act(async () => {
      Array.from(container.querySelectorAll("button"))
        .find((button) => button.textContent === "Off")
        ?.click()
    })
    await act(async () => vi.advanceTimersByTimeAsync(250))
    expect(accepted).toHaveLength(3)
    expect(accepted[2]).toEqual({
      action: "select",
      selection: { mode: "off" },
    })
  } finally {
    await act(async () => root.unmount())
    container.remove()
    vi.useRealTimers()
    vi.unstubAllGlobals()
  }
})

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

test("polling reports pending, failed and recovered targets without claiming stale readback is current", async () => {
  const { backlightStatus } = await import("@/lib/backlight")

  const baseline = backlightStatus.parse({
    setup: "ready",
    reachability: "unreachable",
    intended: { mode: "off" },
    acknowledged: { mode: "tv-sync" },
    observed: {
      power: true,
      brightness: 45,
      mode: "tv-sync",
      observed_at: "2026-10-23T18:00:00Z",
    },
    stale: true,
    command: "failed",
    recovery: "retrying",
    bedtime: { enabled: false, start: "23:00", end: "00:00", brightness: 20 },
    control_source: "manual",
    override_expires_at: null,
    next_transition: null,
  })

  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)

  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)

  async function render(
    status: import("@/lib/backlight").BacklightStatus
  ): Promise<void> {
    await act(async () =>
      root.render(
        <BacklightControls
          data={{ state: "available", status }}
          change={async () => ({ state: "unavailable" })}
        />
      )
    )
  }

  try {
    await render(baseline)
    expect(container.textContent).toContain("Retrying latest selection")
    expect(container.textContent).toContain("Failed; selection saved")
    expect(container.textContent).toContain("Stale / unknown")
    await render({ ...baseline, command: "pending", recovery: "pending" })
    expect(container.textContent).toContain("Pending")
    await render({
      ...baseline,
      command: "acknowledged",
      recovery: "recovered",
      reachability: "reachable",
      stale: false,
      acknowledged: { mode: "off" },
    })
    expect(container.textContent).toContain("Current selection recovered")
    expect(container.textContent).toContain("Fresh")
  } finally {
    await act(async () => root.unmount())
    container.remove()
    vi.unstubAllGlobals()
  }
})

test("native effects are one tap manual selections and readable mode never claims their identity", async () => {
  const { backlightStatus } = await import("@/lib/backlight")

  const status = backlightStatus.parse({
    setup: "ready",
    reachability: "reachable",
    intended: { mode: "tv-sync" },
    acknowledged: null,
    observed: {
      power: true,
      mode: "other",
      brightness: 35,
      observed_at: "2026-10-24T12:00:00Z",
    },
    stale: false,
    command: "idle",
    recovery: "recovered",
    bedtime: { enabled: false, start: "23:00", end: "00:00", brightness: 20 },
    control_source: "automatic",
    override_expires_at: null,
    next_transition: null,
    effects: [
      {
        id: "aurora",
        name: "Aurora",
        description: "Cool waves of green and blue.",
      },
    ],
  })

  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)

  const container = document.createElement("div")

  document.body.append(container)

  const root = createRoot(container)

  const actions: import("@/lib/backlight").BacklightAction[] = []

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
                intended:
                  action.action === "select"
                    ? action.selection
                    : status.intended,
              },
            }
          }}
        />
      )
    )

    const effect = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent?.includes("Aurora")
    )

    expect(effect).toBeDefined()

    await act(async () => effect?.click())

    expect(actions).toEqual([
      {
        action: "select",
        selection: { mode: "effect", effect: "aurora", brightness: 35 },
      },
    ])

    expect(container.textContent).toContain("Requested effectAurora")

    expect(container.textContent).toContain("Observed modeOther")

    const sync = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "TV Sync"
    )

    await act(async () => sync?.click())

    expect(actions.at(-1)).toEqual({
      action: "select",
      selection: { mode: "tv-sync" },
    })
  } finally {
    await act(async () => root.unmount())
    container.remove()

    vi.unstubAllGlobals()
  }
})
