// @vitest-environment happy-dom
import { act } from "react"
import { createRoot } from "react-dom/client"
import { expect, test, vi } from "vitest"
import { BacklightControls } from "@/components/backlight-controls"
import type {
  BacklightAction,
  BacklightResult,
  BacklightStatus,
} from "@/lib/backlight"

// Delivery exits: every call to `change` stays open until the test settles it, so each
// response, rejection and unmount lands at a chosen moment.
type OpenCall = {
  action: BacklightAction
  resolve: (result: BacklightResult) => void
  reject: () => void
}

async function readyStatus(): Promise<BacklightStatus> {
  const { backlightStatus } = await import("@/lib/backlight")

  return backlightStatus.parse({
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
}

async function mountOpenCalls(status: BacklightStatus): Promise<{
  container: HTMLElement
  calls: OpenCall[]
  unmount: () => Promise<void>
}> {
  const calls: OpenCall[] = []
  const container = document.createElement("div")
  document.body.append(container)
  const root = createRoot(container)

  await act(async () =>
    root.render(
      <BacklightControls
        data={{ state: "available", status }}
        change={(action) =>
          new Promise<BacklightResult>((resolve, reject) => {
            calls.push({
              action,
              resolve,
              reject: () => reject(new Error("synthetic transport failure")),
            })
          })
        }
      />
    )
  )

  return {
    container,
    calls,
    unmount: async () => {
      await act(async () => root.unmount())
      container.remove()
    },
  }
}

const acknowledged = (status: BacklightStatus): BacklightResult => ({
  state: "available",
  status: { ...status, command: "acknowledged" },
})

async function tap(container: HTMLElement, label: string): Promise<void> {
  const button = Array.from(container.querySelectorAll("button")).find(
    (candidate) => candidate.textContent === label
  )

  expect(button).toBeDefined()
  await act(async () => button?.click())
}

async function nudgeBrightness(
  container: HTMLElement,
  ...keys: string[]
): Promise<void> {
  for (const key of keys)
    await act(async () => {
      container
        .querySelector('[aria-label="Brightness"]')
        ?.dispatchEvent(
          new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true })
        )
    })
}

const feedback = (container: HTMLElement): string | null | undefined =>
  container.querySelector("[role=status]")?.textContent

const busy = (container: HTMLElement): string | null | undefined =>
  container.querySelector("[aria-busy]")?.getAttribute("aria-busy")

test("a failed change keeps older feedback quiet, drains the latest queued selection, then releases the interaction", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const view = await mountOpenCalls(await readyStatus())

  try {
    await tap(view.container, "TV Sync")
    await tap(view.container, "Color")
    await tap(view.container, "Off")
    expect(view.calls.map((call) => call.action)).toEqual([
      { action: "select", selection: { mode: "tv-sync" } },
    ])

    await act(async () => view.calls[0]?.reject())
    expect(view.calls.map((call) => call.action)).toEqual([
      { action: "select", selection: { mode: "tv-sync" } },
      { action: "select", selection: { mode: "off" } },
    ])
    expect(feedback(view.container)).toBe("Sending your latest selection…")
    expect(busy(view.container)).toBe("true")

    await act(async () => view.calls[1]?.reject())
    expect(feedback(view.container)).toContain("couldn’t confirm")
    expect(view.container.textContent).toContain("No answer")
    expect(busy(view.container)).toBe("false")

    const refresh = view.container.querySelector('[title="Refresh state"]')

    expect(refresh).toBeInstanceOf(HTMLButtonElement)
    expect(refresh instanceof HTMLButtonElement && refresh.disabled).toBe(false)
  } finally {
    await view.unmount()
    vi.unstubAllGlobals()
  }
})

test("an older response does not replace the feedback of an edit still waiting to be sent", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()
  const status = await readyStatus()
  const view = await mountOpenCalls(status)

  try {
    await tap(view.container, "Color")
    await nudgeBrightness(view.container, "Home", "PageUp")
    await act(async () => view.calls[0]?.resolve(acknowledged(status)))
    expect(feedback(view.container)).toBe("Waiting for your latest adjustment…")
    expect(busy(view.container)).toBe("true")
    expect(view.calls).toHaveLength(1)

    await act(async () => vi.advanceTimersByTimeAsync(250))
    expect(view.calls).toHaveLength(2)
    expect(view.calls[1]?.action).toMatchObject({
      action: "select",
      selection: { mode: "color", brightness: 10 },
    })

    await act(async () => view.calls[1]?.resolve(acknowledged(status)))
    expect(feedback(view.container)).toContain("Command acknowledged")
    expect(busy(view.container)).toBe("false")
  } finally {
    await view.unmount()
    vi.useRealTimers()
    vi.unstubAllGlobals()
  }
})

test("unmounting sends nothing more: a waiting edit is cancelled and a response in flight is dropped", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  vi.useFakeTimers()
  const status = await readyStatus()

  try {
    const waiting = await mountOpenCalls(status)

    await nudgeBrightness(waiting.container, "Home", "PageUp")
    await waiting.unmount()
    await act(async () => vi.advanceTimersByTimeAsync(1000))
    expect(waiting.calls).toEqual([])

    const sending = await mountOpenCalls(status)

    await tap(sending.container, "Off")
    await tap(sending.container, "TV Sync")
    await sending.unmount()
    await act(async () => sending.calls[0]?.resolve(acknowledged(status)))
    await act(async () => vi.advanceTimersByTimeAsync(1000))
    expect(sending.calls.map((call) => call.action)).toEqual([
      { action: "select", selection: { mode: "off" } },
    ])
  } finally {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  }
})
