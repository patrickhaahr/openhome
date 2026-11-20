// @vitest-environment happy-dom
import { act } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { BacklightControls } from "@/components/backlight-controls"

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
