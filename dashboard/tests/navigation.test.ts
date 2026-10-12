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
import { Route as WeatherRoute } from "@/routes/weather"
import { Route as WishlistRoute } from "@/routes/wishlist"
import { Route as WatchlistRoute } from "@/routes/watchlist"
import { unavailableWeather } from "@/lib/weather"
import { routeTree } from "@/routeTree.gen"

test("opens focused views through persistent navigation, restores browser history and moves focus to the page", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const originalLoader = HomeRoute.options.loader
  const originalWeatherLoader = WeatherRoute.options.loader
  const originalWishlistLoader = WishlistRoute.options.loader
  const originalWatchlistLoader = WatchlistRoute.options.loader
  WatchlistRoute.options.loader = () => ({
    state: "available",
    owners: [],
    entries: [],
  })
  WishlistRoute.options.loader = () => ({
    state: "unavailable",
    reason: "unavailable",
  })
  WeatherRoute.options.loader = () => unavailableWeather

  HomeRoute.options.loader = () => ({
    weather: Promise.resolve(unavailableWeather),
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
      "Wishlists unavailable"
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

    const watchlist = document.querySelector<HTMLAnchorElement>(
      'nav a[href="/watchlist"]'
    )

    if (watchlist === null) throw new Error("Expected Watchlist navigation")

    await act(async () => {
      watchlist.click()
      await router.load()
    })
    expect(document.querySelector("h1")?.textContent).toBe("Watchlist")
    expect(document.activeElement).toBe(document.querySelector("h1"))
    expect(
      document.querySelector('nav a[aria-current="page"]')?.textContent
    ).toBe("Watchlist")
    expect(document.querySelector("main")?.textContent).toContain(
      "Add a household member in Wishlist"
    )
    await act(async () => {
      history.back()
      await router.load()
    })

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
      "Weather unavailable"
    )
  } finally {
    await act(async () => {
      root.unmount()
    })
    vi.unstubAllGlobals()

    if (originalLoader !== undefined) HomeRoute.options.loader = originalLoader

    if (originalWatchlistLoader !== undefined)
      WatchlistRoute.options.loader = originalWatchlistLoader

    if (originalWeatherLoader !== undefined)
      WeatherRoute.options.loader = originalWeatherLoader

    if (originalWishlistLoader !== undefined)
      WishlistRoute.options.loader = originalWishlistLoader
  }
})
