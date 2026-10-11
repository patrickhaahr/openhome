// @vitest-environment happy-dom

import { act, createElement } from "react"
import { createRoot, type Root } from "react-dom/client"
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
  useRouterState,
} from "@tanstack/react-router"
import { afterEach, beforeEach, expect, test, vi } from "vitest"
import { LightControls } from "@/components/light-controls"
import type { ConnectionState } from "@/lib/connection"
import type { LightCommand, LightCommandResult } from "@/lib/lights"

type Harness = {
  commands: LightCommand[]
  healthChecks: () => number
  resolve: (result: LightCommandResult) => Promise<void>
  reject: (error: Error) => Promise<void>
}

let container: HTMLDivElement

let root: Root

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => {
    root.unmount()
  })
  container.remove()
  vi.unstubAllGlobals()
})

/** Render Home's light card in a router whose health check returns each state in turn. */
async function renderLights(
  ...connectionStates: [ConnectionState, ...ConnectionState[]]
): Promise<Harness> {
  const commands: LightCommand[] = []
  let healthChecks = 0
  let settle: Pick<Harness, "resolve" | "reject"> | undefined

  function sendCommand({
    data,
  }: {
    data: LightCommand
  }): Promise<LightCommandResult> {
    commands.push(data)

    return new Promise((resolve, reject) => {
      settle = {
        resolve: async (result) => {
          await act(async () => resolve(result))
        },
        reject: async (error) => {
          await act(async () => reject(error))
        },
      }
    })
  }

  let connectionState = connectionStates[0]

  const rootRoute = createRootRoute({
    // Each load is one health check; a new state re-renders Home like the real route loader.
    loader: () => {
      connectionState = connectionStates[healthChecks] ?? connectionState
      healthChecks += 1

      return { connectionState }
    },
    component: function Home() {
      // Re-render after each check, as Home does when its loader data changes.
      useRouterState({ select: (state) => state.matches[0]?.updatedAt })

      return createElement(LightControls, { connectionState, sendCommand })
    },
  })

  const router = createRouter({
    routeTree: rootRoute,
    history: createMemoryHistory(),
  })

  await act(async () => {
    await router.load()
    root.render(createElement(RouterProvider, { router }))
  })

  function pendingCommand(): Pick<Harness, "resolve" | "reject"> {
    if (settle === undefined) throw new Error("Expected a pending command")

    return settle
  }

  return {
    commands,
    healthChecks: () => healthChecks,
    resolve: (result) => pendingCommand().resolve(result),
    reject: (error) => pendingCommand().reject(error),
  }
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

function linkTexts(): string[] {
  return Array.from(container.querySelectorAll("a")).map((link) =>
    link.textContent.trim()
  )
}

async function tap(label: string): Promise<void> {
  await act(async () => {
    button(label).click()
  })
}

test("sends one command for queued and pending taps and announces acceptance without claiming power", async () => {
  const lights = await renderLights("connected")

  await act(async () => {
    button("Turn on").click()
    button("Turn on").click()
    button("Turn off").click()
  })
  expect(lights.commands).toEqual(["on"])
  expect(status()).toBe("Sending on command…")
  expect(button("Turn on").disabled).toBe(true)
  expect(button("Turn off").disabled).toBe(true)

  await tap("Turn off")
  expect(lights.commands).toEqual(["on"])

  await lights.resolve({ state: "accepted" })
  expect(status()).toBe("On command accepted by OpenHome.")
  expect(container.textContent).toContain("Observed state Unknown")
  expect(button("Turn off").disabled).toBe(false)

  await tap("Turn off")
  await lights.resolve({ state: "accepted" })
  expect(status()).toBe("Off command accepted by OpenHome.")
  expect(container.textContent).toContain("Observed state Unknown")
  expect(lights.commands).toEqual(["on", "off"])
})

test("recovers from failed commands and keeps private RPC errors out of the page", async () => {
  const lights = await renderLights("connected")

  await tap("Turn off")
  await lights.resolve({ state: "failed", reason: "missing-device" })
  expect(status()).toContain("Lights are not configured.")
  expect(button("Turn off").disabled).toBe(false)

  await tap("Turn off")
  await lights.resolve({ state: "failed", reason: "unavailable" })
  expect(status()).toContain("Command not confirmed.")

  await tap("Turn off")
  await lights.reject(new Error("synthetic-private-RPC-error"))
  expect(status()).toContain("The lights may have changed.")
  expect(container.textContent).not.toContain("synthetic-private")
  expect(button("Turn on").disabled).toBe(false)
  expect(lights.commands).toEqual(["off", "off", "off"])
  expect(container.textContent).toContain("Observed state Unknown")
})

test("links to Connection after a command reveals a setup problem", async () => {
  const lights = await renderLights("connected")
  expect(linkTexts()).not.toContain("Open Connection")

  await tap("Turn on")
  await lights.resolve({ state: "failed", reason: "rejected-credential" })
  expect(status()).toContain("OpenHome rejected the API key.")
  expect(linkTexts()).toContain("Open Connection")
})

test("pauses commands while disconnected without suggesting the lights changed, then recovers on a new check", async () => {
  const lights = await renderLights("unreachable", "connected")

  expect(status()).toBe(
    "OpenHome is unreachable. Light commands are paused until it responds."
  )
  expect(status()).not.toContain("may have changed")
  expect(button("Turn on").disabled).toBe(true)
  expect(linkTexts()).toContain("Open Connection")

  await tap("Turn on")
  expect(lights.commands).toEqual([])

  await tap("Check again")
  expect(lights.healthChecks()).toBe(2)
  expect(status()).toBe("Ready to send a command.")
  expect(button("Turn on").disabled).toBe(false)
  expect(container.textContent).not.toContain("Check again")
})

test("keeps the focused controls view reachable from Home", async () => {
  await renderLights("connected")

  const link = Array.from(container.querySelectorAll("a")).find(
    (element) => element.textContent.trim() === "All controls"
  )

  expect(link?.getAttribute("href")).toBe("/controls")
})
