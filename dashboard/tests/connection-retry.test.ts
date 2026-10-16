// @vitest-environment happy-dom

import { act, createElement } from "react"
import { createRoot } from "react-dom/client"
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router"
import { expect, test, vi } from "vitest"
import type { ConnectionHealth } from "@/lib/connection"
import { Route } from "@/routes/connection"
import { routeTree } from "@/routeTree.gen"

type PendingHealth = { resolve?: (health: ConnectionHealth) => void }

test("keeps retry disabled and announces checking until the refreshed health arrives", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)

  const initial: ConnectionHealth = {
    state: "connected",
    checkedAt: "2026-10-11T10:00:00.000Z",
  }

  const refreshed: ConnectionHealth = {
    state: "unreachable",
    checkedAt: "2026-10-11T10:01:00.000Z",
  }

  const deferred: PendingHealth = {}

  const delayed = new Promise<ConnectionHealth>((resolve) => {
    deferred.resolve = resolve
  })

  let response = Promise.resolve(initial)

  const originalLoader = Route.options.loader

  Route.options.loader = () => response

  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: ["/connection"] }),
    isServer: false,
  })

  const root = createRoot(document)

  try {
    await router.load()
    await act(async () => {
      root.render(createElement(RouterProvider, { router }))
    })

    const button = [...document.querySelectorAll("button")].find((element) =>
      element.textContent?.includes("Check again")
    )

    if (button === undefined)
      throw new Error("Expected a rendered retry button")

    expect(document.querySelector("[role=status]")?.textContent).toContain(
      "Connected"
    )
    response = delayed
    await act(async () => {
      button.click()
    })

    expect(button.disabled).toBe(true)
    expect(button.textContent).toContain("Checking…")
    expect(document.querySelector("[role=status]")?.textContent).toContain(
      "Checking connection…"
    )

    await act(async () => {
      deferred.resolve?.(refreshed)
      await delayed
    })

    expect(button.disabled).toBe(false)
    expect(button.textContent).toContain("Check again")
    expect(document.querySelector("[role=status]")?.textContent).toContain(
      "Unreachable"
    )
    expect(
      document.querySelector("[role=status]")?.querySelector("time")?.dateTime
    ).toBe(refreshed.checkedAt)
  } finally {
    await act(async () => {
      deferred.resolve?.(refreshed)
      await delayed
      root.unmount()
    })
    vi.unstubAllGlobals()

    if (originalLoader !== undefined) {
      Route.options.loader = originalLoader
    }
  }
})
