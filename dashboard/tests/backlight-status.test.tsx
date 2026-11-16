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
