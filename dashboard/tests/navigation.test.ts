// @vitest-environment happy-dom

import { act, createElement } from "react"
import { createRoot } from "react-dom/client"
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router"
import { expect, test, vi } from "vitest"
import { Route as HomeRoute } from "@/routes/index"
import { routeTree } from "@/routeTree.gen"

test("opens focused views through persistent navigation, restores browser history and moves focus to the page", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const originalLoader = HomeRoute.options.loader

  HomeRoute.options.loader = () => ({
    health: { state: "connected", checkedAt: "2026-10-11T10:00:00.000Z" },
    tvRemote: Promise.resolve({ state: "available", commands: ["power"] }),
    speakerRemote: Promise.resolve({ state: "available", commands: ["power"] }),
  })
  const history = createMemoryHistory({ initialEntries: ["/"] })
  const router = createRouter({ routeTree, history, isServer: false })
  const root = createRoot(document)

  try {
    await router.load()
    await act(async () => {
      root.render(createElement(RouterProvider, { router }))
    })

    const wishlist = document.querySelector<HTMLAnchorElement>(
      'nav a[href="/wishlist"]'
    )

    if (wishlist === null) throw new Error("Expected Wishlist navigation")

    await act(async () => {
      wishlist.click()
      await router.load()
    })
    expect(router.state.location.pathname).toBe("/wishlist")
    expect(document.querySelector("h1")?.textContent).toBe("Wishlist")
    expect(document.activeElement).toBe(document.querySelector("h1"))
    expect(
      document.querySelector('nav a[aria-current="page"]')?.textContent
    ).toBe("Wishlist")
    expect(document.querySelector("main")?.textContent).toContain(
      "Household users and their lists are unavailable."
    )

    const photos = document.querySelector<HTMLAnchorElement>(
      'nav a[href="/photos"]'
    )

    if (photos === null)
      throw new Error("Expected persistent Photos navigation")

    await act(async () => {
      photos.click()
      await router.load()
    })
    expect(document.querySelector("h1")?.textContent).toBe("Photos")

    await act(async () => {
      history.back()
      await router.load()
    })
    expect(document.querySelector("h1")?.textContent).toBe("Wishlist")
    expect(document.activeElement).toBe(document.querySelector("h1"))

    await act(async () => {
      history.back()
      await router.load()
    })
    expect(document.querySelector("h1")?.textContent).toBe("Home")

    const weather = document.querySelector<HTMLAnchorElement>(
      'a[aria-label="Open Weather"]'
    )

    if (weather === null) throw new Error("Expected a Weather card link")

    await act(async () => {
      weather.click()
      await router.load()
    })
    expect(document.querySelector("h1")?.textContent).toBe("Weather")
    expect(document.querySelector("main")?.textContent).toContain(
      "No current conditions or forecast are available."
    )
  } finally {
    await act(async () => {
      root.unmount()
    })
    vi.unstubAllGlobals()

    if (originalLoader !== undefined) HomeRoute.options.loader = originalLoader
  }
})
