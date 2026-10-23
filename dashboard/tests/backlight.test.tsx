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
