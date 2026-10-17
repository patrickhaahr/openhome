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
import { Route as ListsRoute } from "@/routes/lists"
import { Route as RecoveryRoute } from "@/routes/recovery"
import { Route as WorkoutRoute } from "@/routes/workout"
import { unavailableWeather } from "@/lib/weather"
import { routeTree } from "@/routeTree.gen"

test("opens focused views through persistent navigation, restores browser history and moves focus to the page", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const originalLoader = HomeRoute.options.loader
  const originalWeatherLoader = WeatherRoute.options.loader
  const originalListsLoader = ListsRoute.options.loader
  const originalWorkoutLoader = WorkoutRoute.options.loader
  const originalRecoveryLoader = RecoveryRoute.options.loader
  const unavailable = { state: "unavailable", reason: "unreachable" } as const
  WorkoutRoute.options.loader = ({ deps }) =>
    deps.view === "body"
      ? {
          view: "body",
          data: {
            state: "available",
            profile: { height_cm: null, sex: null, hr_max_bpm: null },
            history: [],
          },
        }
      : { view: "log", data: unavailable }
  RecoveryRoute.options.loader = ({ deps }) => ({
    day: deps.day ?? "2026-01-15",
    today: "2026-01-15",
    recovery: unavailable,
    week: [],
    trends: {
      hrv: unavailable,
      resting_heart_rate: unavailable,
      sleep_duration: unavailable,
    },
    context: unavailable,
    detail: { state: "none" },
  })
  ListsRoute.options.loader = ({ deps }) =>
    deps.tab === "watch"
      ? {
          tab: "watch",
          data: { state: "available", owners: [], entries: [] },
        }
      : { tab: "wish", data: { state: "unavailable", reason: "unavailable" } }
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

    // Wishes and watching share one destination; the old pages are not in the navigation.
    expect(document.querySelector('nav a[href="/wishlist"]')).toBeNull()
    expect(document.querySelector('nav a[href="/watchlist"]')).toBeNull()

    const lists = document.querySelector<HTMLAnchorElement>(
      'nav a[href="/lists"]'
    )

    if (lists === null) throw new Error("Expected Lists navigation")

    await act(async () => {
      lists.click()
      await router.load()
    })
    expect(router.state.location.pathname).toBe("/lists")
    expect(document.querySelector("h1")?.textContent).toBe("Lists")
    expect(document.activeElement).toBe(document.querySelector("h1"))
    expect(
      document.querySelector('nav a[aria-current="page"]')?.textContent
    ).toBe("Lists")
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
    expect(document.querySelector("h1")?.textContent).toBe("Lists")
    expect(document.activeElement).toBe(document.querySelector("h1"))

    const watch = document.querySelector<HTMLAnchorElement>(
      'nav[aria-label="Lists"] a[href*="tab=watch"]'
    )

    if (watch === null) throw new Error("Expected the Watch tab")

    await act(async () => {
      watch.click()
      await router.load()
    })
    expect(router.state.location.search).toMatchObject({ tab: "watch" })
    expect(
      document.querySelector('nav[aria-label="Lists"] a[aria-current="page"]')
        ?.textContent
    ).toBe("Watch")
    expect(document.querySelector("main")?.textContent).toContain(
      "Household members are added under Wish."
    )

    await act(async () => {
      await router.navigate({ to: "/" })
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
      "Weather unavailable"
    )

    const recovery = document.querySelector<HTMLAnchorElement>(
      'nav a[href="/recovery"]'
    )

    if (recovery === null) throw new Error("Expected Recovery navigation")

    await act(async () => {
      recovery.click()
      await router.load()
    })
    expect(document.querySelector("h1")?.textContent).toBe("Recovery")
    expect(
      document.querySelector('nav a[aria-current="page"]')?.textContent
    ).toBe("Recovery")
    expect(document.querySelector("main")?.textContent).toContain(
      "Recovery day unavailable"
    )

    const workout = document.querySelector<HTMLAnchorElement>(
      'nav a[href="/workout"]'
    )

    if (workout === null) throw new Error("Expected Workout navigation")

    await act(async () => {
      workout.click()
      await router.load()
    })
    expect(document.querySelector("h1")?.textContent).toBe("Workout")
    expect(
      document.querySelector('nav a[aria-current="page"]')?.textContent
    ).toBe("Workout")
    expect(
      document.querySelector(
        'nav[aria-label="Workout views"] a[aria-current="page"]'
      )?.textContent
    ).toBe("Log")
    expect(document.querySelector("main")?.textContent).toContain(
      "Workouts unavailable"
    )

    const body = document.querySelector<HTMLAnchorElement>(
      'nav[aria-label="Workout views"] a[href*="view=body"]'
    )

    if (body === null) throw new Error("Expected Body navigation")

    await act(async () => {
      body.click()
      await router.load()
    })
    expect(router.state.location.search).toMatchObject({ view: "body" })
    expect(
      document.querySelector(
        'nav[aria-label="Workout views"] a[aria-current="page"]'
      )?.textContent
    ).toBe("Body")
    expect(document.querySelector("main")?.textContent).toContain(
      "No body weight recorded in this date range."
    )

    await act(async () => {
      history.back()
      await router.load()
    })
    expect(document.querySelector("main")?.textContent).toContain(
      "Workouts unavailable"
    )

    // The retired Today view and old day links land on Recovery, keeping the day.
    await act(async () => {
      await router.navigate({
        to: "/workout",
        search: { view: "today", day: "2026-01-12" },
      })
    })
    expect(router.state.location.pathname).toBe("/recovery")
    expect(router.state.location.search).toEqual({ day: "2026-01-12" })
    await act(async () => {
      await router.navigate({ to: "/workout", search: { day: "2026-01-11" } })
    })
    expect(router.state.location.pathname).toBe("/recovery")
    expect(router.state.location.search).toEqual({ day: "2026-01-11" })
  } finally {
    await act(async () => {
      root.unmount()
    })
    vi.unstubAllGlobals()

    if (originalLoader !== undefined) HomeRoute.options.loader = originalLoader

    if (originalListsLoader !== undefined)
      ListsRoute.options.loader = originalListsLoader

    if (originalWeatherLoader !== undefined)
      WeatherRoute.options.loader = originalWeatherLoader

    if (originalWorkoutLoader !== undefined)
      WorkoutRoute.options.loader = originalWorkoutLoader

    if (originalRecoveryLoader !== undefined)
      RecoveryRoute.options.loader = originalRecoveryLoader
  }
})
