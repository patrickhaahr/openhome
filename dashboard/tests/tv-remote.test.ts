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
import { TvRemote } from "@/components/tv-remote"
import type { ConnectionState } from "@/lib/connection"
import type { TvCommand, TvCommandResult, TvRemoteState } from "@/lib/tv"

type Check = { connection: ConnectionState; remote: TvRemoteState }

type Harness = {
  commands: TvCommand[]
  checks: () => number
  /** Re-run the page's checks, as any Check again on the page does. */
  recheck: () => Promise<void>
  resolve: (result: TvCommandResult) => Promise<void>
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

const synthetic: TvRemoteState = {
  state: "available",
  commands: ["power", "volume-up", "up", "ok", "hdmi-1", "1", "0"],
}

/** Render the TV remote in a router whose checks return each health and discovery outcome in turn. */
async function renderTv(
  layout: "quick" | "full",
  ...checks: [Check, ...Check[]]
): Promise<Harness> {
  const commands: TvCommand[] = []
  let loads = 0
  let settle: Pick<Harness, "resolve" | "reject"> | undefined

  function sendCommand({
    data,
  }: {
    data: TvCommand
  }): Promise<TvCommandResult> {
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

  let current = checks[0]
  let remote = Promise.resolve(current.remote)

  const rootRoute = createRootRoute({
    // Each load is one health check and discovery, like the real route loaders.
    loader: () => {
      current = checks[loads] ?? current
      remote = Promise.resolve(current.remote)
      loads += 1

      return { connectionState: current.connection }
    },
    component: function Page() {
      // Re-render after each check, as pages do when their loader data changes.
      useRouterState({ select: (state) => state.matches[0]?.updatedAt })

      return createElement(TvRemote, {
        layout,
        connectionState: current.connection,
        remote,
        sendCommand,
      })
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
    checks: () => loads,
    recheck: async () => {
      await act(async () => {
        await router.invalidate({ sync: true })
      })
    },
    resolve: (result) => pendingCommand().resolve(result),
    reject: (error) => pendingCommand().reject(error),
  }
}

function buttonLabels(): string[] {
  return Array.from(container.querySelectorAll("button")).map(
    (element) => element.textContent
  )
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

test("Home shows only reported common commands and links to the full remote", async () => {
  await renderTv("quick", { connection: "connected", remote: synthetic })

  expect(buttonLabels()).toEqual(["Power", "Volume up"])
  expect(status()).toBe("Ready to send a command.")
  expect(
    Array.from(container.querySelectorAll("a"))
      .find((link) => link.textContent.trim() === "Full remote")
      ?.getAttribute("href")
  ).toBe("/controls")
})

test("the full remote groups every reported command and omits unreported ones", async () => {
  await renderTv("full", { connection: "connected", remote: synthetic })

  const groups = Array.from(
    container.querySelectorAll(".tv-group[role=group]")
  ).map((group) => ({
    label: group.getAttribute("aria-label"),
    buttons: Array.from(group.querySelectorAll("button")).map(
      (element) => element.textContent
    ),
  }))

  expect(groups).toEqual([
    { label: "Power and input", buttons: ["Power", "HDMI 1"] },
    { label: "Volume", buttons: ["Volume up"] },
    { label: "Navigation", buttons: ["Up", "OK"] },
    { label: "Numbers", buttons: ["1", "0"] },
  ])
  expect(linkTexts()).not.toContain("Full remote")
})

test("sends one command for queued and pending taps and never claims TV power or volume", async () => {
  const tv = await renderTv("quick", {
    connection: "connected",
    remote: synthetic,
  })

  await act(async () => {
    button("Volume up").click()
    button("Volume up").click()
    button("Power").click()
  })
  expect(tv.commands).toEqual(["volume-up"])
  expect(status()).toBe("Sending Volume up command…")
  expect(button("Power").disabled).toBe(true)

  await tap("Power")
  expect(tv.commands).toEqual(["volume-up"])

  await tv.resolve({ state: "accepted" })
  expect(status()).toBe("Volume up command accepted by OpenHome.")
  expect(container.textContent).toContain("Power and volume Not reported")
  expect(container.querySelector("[aria-pressed]")).toBeNull()

  await tap("Power")
  await tv.resolve({ state: "accepted" })
  expect(status()).toBe("Power command accepted by OpenHome.")
  expect(container.textContent).toContain("Power and volume Not reported")
  expect(tv.commands).toEqual(["volume-up", "power"])
})

test("recovers from failed commands and keeps private RPC errors out of the page", async () => {
  const tv = await renderTv("quick", {
    connection: "connected",
    remote: synthetic,
  })

  await tap("Power")
  await tv.resolve({ state: "failed", reason: "unavailable" })
  expect(status()).toContain("Command not confirmed.")
  expect(button("Power").disabled).toBe(false)

  await tap("Power")
  await tv.reject(new Error("synthetic-private-RPC-error"))
  expect(status()).toContain("The TV may have responded.")
  expect(container.textContent).not.toContain("synthetic-private")
  expect(button("Power").disabled).toBe(false)
  expect(tv.commands).toEqual(["power", "power"])
})

test("refreshes stale discovery after the remote rejects a command, without reloading", async () => {
  const tv = await renderTv(
    "quick",
    { connection: "connected", remote: synthetic },
    {
      connection: "connected",
      remote: { state: "available", commands: ["volume-up"] },
    }
  )

  await tap("Power")
  await tv.resolve({ state: "failed", reason: "unsupported-command" })
  expect(status()).toContain("no longer reports this command")

  await tap("Check again")
  expect(tv.checks()).toBe(2)
  expect(buttonLabels()).toEqual(["Volume up"])
  expect(status()).toBe("Ready to send a command.")
})

test("explains a missing TV remote without offering buttons, then recovers on a new check", async () => {
  const tv = await renderTv(
    "quick",
    {
      connection: "connected",
      remote: { state: "unavailable", reason: "missing-device" },
    },
    { connection: "connected", remote: synthetic }
  )

  expect(status()).toContain("The TV remote is not configured.")
  expect(status()).not.toContain("may have responded")
  expect(buttonLabels()).toEqual(["Check again"])
  expect(linkTexts()).not.toContain("Open Connection")
  expect(linkTexts()).not.toContain("Full remote")

  await tap("Check again")
  expect(tv.checks()).toBe(2)
  expect(buttonLabels()).toEqual(["Power", "Volume up"])
})

test("explains when OpenHome reports no supported TV commands", async () => {
  await renderTv("full", {
    connection: "connected",
    remote: { state: "available", commands: [] },
  })

  expect(buttonLabels()).toEqual([])
  expect(status()).toBe("OpenHome reports no TV commands this remote supports.")
})

test("pauses commands while disconnected and links to Connection", async () => {
  const tv = await renderTv("quick", {
    connection: "rejected-credential",
    remote: { state: "unavailable", reason: "rejected-credential" },
  })

  expect(status()).toBe(
    "OpenHome rejected the API key. TV commands are paused until it is fixed."
  )
  expect(linkTexts()).toContain("Open Connection")
  expect(buttonLabels()).toEqual(["Check again"])
  expect(tv.commands).toEqual([])
})

test("keeps a pending command guarded when the page re-checks before it settles", async () => {
  const tv = await renderTv(
    "quick",
    { connection: "connected", remote: synthetic },
    { connection: "connected", remote: synthetic }
  )

  await tap("Power")
  await tv.recheck()
  expect(tv.checks()).toBe(2)
  expect(status()).toBe("Sending Power command…")
  expect(button("Power").disabled).toBe(true)

  await tap("Volume up")
  expect(tv.commands).toEqual(["power"])

  await tv.resolve({ state: "accepted" })
  expect(status()).toBe("Power command accepted by OpenHome.")
})
