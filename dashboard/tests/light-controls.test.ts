// @vitest-environment happy-dom

import { act, createElement } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { LightControls } from "@/components/light-controls"
import type { LightCommand, LightCommandResult } from "@/lib/lights"

type PendingCommand = {
  resolve?: (result: LightCommandResult) => void
  reject?: (error: Error) => void
}

test("announces commands, blocks queued and pending taps, and keeps power unknown across accepted and failed results", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)
  const commands: LightCommand[] = []

  const pending: PendingCommand = {}

  function sendCommand({
    data,
  }: {
    data: LightCommand
  }): Promise<LightCommandResult> {
    commands.push(data)

    return new Promise((resolve, reject) => {
      pending.resolve = resolve
      pending.reject = reject
    })
  }

  function button(label: string): HTMLButtonElement {
    const found = Array.from(container.querySelectorAll("button")).find(
      (element) => element.textContent === label
    )

    if (found === undefined) throw new Error(`Expected ${label} button`)

    return found
  }

  function status(): string {
    return container.querySelector("[role=status]")?.textContent ?? ""
  }

  try {
    await act(async () => {
      root.render(
        createElement(LightControls, {
          connectionState: "connected",
          sendCommand,
        })
      )
    })
    await act(async () => {
      button("Turn on").click()
      button("Turn on").click()
      button("Turn off").click()
    })
    expect(commands).toEqual(["on"])
    expect(status()).toContain("Sending on command…")
    expect(button("Turn on").disabled).toBe(true)
    expect(button("Turn off").disabled).toBe(true)
    expect(container.textContent).toContain("Observed state Unknown")

    await act(async () => {
      button("Turn off").click()
    })
    expect(commands).toEqual(["on"])

    await act(async () => {
      pending.resolve?.({ state: "accepted" })
    })
    expect(status()).toContain("On command accepted by OpenHome.")
    expect(container.textContent).toContain("Observed state Unknown")
    expect(button("Turn off").disabled).toBe(false)

    await act(async () => {
      button("Turn off").click()
    })
    await act(async () => {
      pending.resolve?.({ state: "failed", reason: "missing-device" })
    })
    expect(status()).toContain("Lights are not configured.")
    expect(status()).toContain("then try again")
    expect(button("Turn off").disabled).toBe(false)

    await act(async () => {
      button("Turn off").click()
    })
    await act(async () => {
      pending.resolve?.({ state: "failed", reason: "unavailable" })
    })
    expect(status()).toContain("Command not confirmed.")
    expect(button("Turn on").disabled).toBe(false)

    await act(async () => {
      button("Turn off").click()
    })
    await act(async () => {
      pending.reject?.(new Error("synthetic-private-RPC-error"))
    })
    expect(status()).toContain("The lights may have changed.")
    expect(container.textContent).not.toContain("synthetic-private")
    expect(button("Turn off").disabled).toBe(false)
    expect(commands).toEqual(["on", "off", "off", "off"])

    await act(async () => {
      button("Turn off").click()
    })
    await act(async () => {
      pending.resolve?.({ state: "accepted" })
    })
    expect(status()).toContain("Off command accepted by OpenHome.")
    expect(container.textContent).toContain("Observed state Unknown")
  } finally {
    await act(async () => {
      pending.resolve?.({ state: "accepted" })
      root.unmount()
    })
    container.remove()
    vi.unstubAllGlobals()
  }
})
