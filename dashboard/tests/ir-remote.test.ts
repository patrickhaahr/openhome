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
import type { IrRemoteProps } from "@/components/ir-remote"
import { SpeakerRemote } from "@/components/speaker-remote"
import { TvRemote } from "@/components/tv-remote"
import type { ControlLayout } from "@/lib/command-feedback"
import type { ConnectionState } from "@/lib/connection"
import type { IrCommandResult, IrRemoteState } from "@/lib/ir"
import type { SpeakerCommand } from "@/lib/speaker"
import type { TvCommand } from "@/lib/tv"

type Check<C extends string> = {
  connection: ConnectionState
  remote: IrRemoteState<C>
}

type Harness<C extends string> = {
  commands: C[]
  checks: () => number
  /** Re-run the page's checks, as any Check again on the page does. */
  recheck: () => Promise<void>
  resolve: (result: IrCommandResult) => Promise<void>
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

const synthetic: IrRemoteState<TvCommand> = {
  state: "available",
  commands: ["power", "volume-up", "up", "ok", "hdmi-1", "1", "0"],
}

/** Render a remote in a router whose checks return each health and discovery outcome in turn. */
async function renderRemote<C extends string>(
  Remote: (props: IrRemoteProps<C>) => React.JSX.Element,
  layout: ControlLayout,
  ...checks: [Check<C>, ...Check<C>[]]
): Promise<Harness<C>> {
  const commands: C[] = []
  let loads = 0
  let settle: Pick<Harness<C>, "resolve" | "reject"> | undefined

  function sendCommand({ data }: { data: C }): Promise<IrCommandResult> {
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

      return createElement(Remote, {
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

  function pendingCommand(): Pick<Harness<C>, "resolve" | "reject"> {
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
  await renderRemote(TvRemote, "quick", {
    connection: "connected",
    remote: synthetic,
  })

  expect(buttonLabels()).toEqual(["Power", "Volume up"])
  expect(status()).toBe("Ready to send a command.")
  expect(
    Array.from(container.querySelectorAll("a"))
      .find((link) => link.textContent.trim() === "Full remote")
      ?.getAttribute("href")
  ).toBe("/controls")
})

test("the full remote groups every reported command and omits unreported ones", async () => {
  await renderRemote(TvRemote, "full", {
    connection: "connected",
    remote: synthetic,
  })

  const groups = Array.from(
    container.querySelectorAll(".remote-group[role=group]")
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
  const tv = await renderRemote(TvRemote, "quick", {
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
  const tv = await renderRemote(TvRemote, "quick", {
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
  const tv = await renderRemote(
    TvRemote,
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
  const tv = await renderRemote(
    TvRemote,
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
  await renderRemote(TvRemote, "full", {
    connection: "connected",
    remote: { state: "available", commands: [] },
  })

  expect(buttonLabels()).toEqual([])
  expect(status()).toBe("OpenHome reports no TV commands this remote supports.")
})

test("pauses commands while disconnected and links to Connection", async () => {
  const tv = await renderRemote(TvRemote, "quick", {
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
  const tv = await renderRemote(
    TvRemote,
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

const syntheticSpeaker: IrRemoteState<SpeakerCommand> = {
  state: "available",
  commands: ["power", "bluetooth", "optical", "volume-up"],
}

test("the speaker shows reported input and volume commands on Home and groups them on the full remote", async () => {
  await renderRemote(SpeakerRemote, "quick", {
    connection: "connected",
    remote: syntheticSpeaker,
  })
  expect(buttonLabels()).toEqual(["Bluetooth", "Optical", "Volume up"])
  expect(linkTexts()).toContain("Full remote")

  await renderRemote(SpeakerRemote, "full", {
    connection: "connected",
    remote: syntheticSpeaker,
  })

  const groups = Array.from(
    container.querySelectorAll(".remote-group[role=group]")
  ).map((group) => ({
    label: group.getAttribute("aria-label"),
    buttons: Array.from(group.querySelectorAll("button")).map(
      (element) => element.textContent
    ),
  }))

  expect(groups).toEqual([
    { label: "Power and input", buttons: ["Power", "Bluetooth", "Optical"] },
    { label: "Volume", buttons: ["Volume up"] },
  ])
})

test("sends one speaker command for a burst of taps and never claims power, input or volume", async () => {
  const speaker = await renderRemote(SpeakerRemote, "quick", {
    connection: "connected",
    remote: syntheticSpeaker,
  })

  await act(async () => {
    button("Optical").click()
    button("Optical").click()
    button("Volume up").click()
  })
  expect(speaker.commands).toEqual(["optical"])
  expect(status()).toBe("Sending Optical command…")
  expect(button("Bluetooth").disabled).toBe(true)

  await speaker.resolve({ state: "accepted" })
  expect(status()).toBe("Optical command accepted by OpenHome.")
  expect(container.textContent).toContain("Observed state Not reported")
  expect(container.textContent).toContain(
    "does not report whether the speaker is on, its input, its volume or what is playing"
  )
  expect(container.querySelector("[aria-pressed]")).toBeNull()

  await tap("Volume up")
  await speaker.reject(new Error("synthetic-private-RPC-error"))
  expect(status()).toContain("The speaker may have responded.")
  expect(container.textContent).not.toContain("synthetic-private")
  expect(button("Volume up").disabled).toBe(false)
})

test("explains a missing speaker remote without offering buttons", async () => {
  await renderRemote(SpeakerRemote, "quick", {
    connection: "connected",
    remote: { state: "unavailable", reason: "missing-device" },
  })

  expect(status()).toContain("The speaker remote is not configured.")
  expect(buttonLabels()).toEqual(["Check again"])
  expect(linkTexts()).not.toContain("Full remote")
})
