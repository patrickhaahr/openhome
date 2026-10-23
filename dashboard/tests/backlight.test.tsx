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

  async function edit(label: string, value: string): Promise<void> {
    const field = container.querySelector(`input[aria-label="${label}"]`)
    expect(field).toBeInstanceOf(HTMLInputElement)

    if (!(field instanceof HTMLInputElement))
      throw new Error(`Missing ${label}`)
    await act(async () => {
      Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value"
      )?.set?.call(field, value)
      field.dispatchEvent(new Event("input", { bubbles: true }))
      field.dispatchEvent(new Event("change", { bubbles: true }))
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
    await edit("RGB color", "#ff8000")
    await edit("Brightness", "10")
    await edit("Brightness", "20")
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
    await edit("RGB color", "#00ff00")
    await edit("RGB color", "#0000ff")
    await edit("Brightness", "30")
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
    await edit("Brightness", "60")
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
