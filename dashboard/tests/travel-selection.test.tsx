// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}

import { act, createElement } from "react"
import { createRoot } from "react-dom/client"
import {
  createMemoryHistory,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router"
import { expect, test, vi } from "vitest"
import { Route as TravelRoute } from "@/routes/travel"
import { routeTree } from "@/routeTree.gen"
import { readMapOrigin } from "@/lib/map.server"
import {
  generateLoops,
  readActiveRoute,
  selectActiveRoute,
} from "@/lib/route-loop.server"
import { startAxumFixture } from "./support/axum-fixture"

test("a refreshed Travel selection resets the displayed map alongside its summary", async () => {
  const api = await startAxumFixture()
  vi.stubEnv("OPENHOME_API_URL", api.url)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  const originalLoader = TravelRoute.options.loader
  const root = createRoot(document)

  // The fixture supplies the real authenticated transport behind Start's RPC boundary.
  TravelRoute.options.loader = async () => ({
    map: await readMapOrigin(),
    active: await readActiveRoute(),
  })

  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: ["/travel"] }),
    isServer: false,
  })

  try {
    const generated = await generateLoops({
      activity: "walking",
      target: { kind: "distance", meters: 5000 },
      step_length_cm: 75,
    })

    if (generated.state !== "generated") throw new Error("Expected loops")

    const [first, second] = generated.plan.loops

    if (first === undefined || second === undefined)
      throw new Error("Expected alternatives")

    await selectActiveRoute({ route_id: first.id })
    await router.load()
    await act(async () =>
      root.render(createElement(RouterProvider, { router }))
    )

    const show = Array.from(document.querySelectorAll("button")).find(
      (button) => button.textContent === "Show active route on map"
    )

    if (show === undefined) throw new Error("Expected map action")

    await act(async () => show.click())
    expect(document.querySelector("figure")?.textContent).toContain(
      "Showing active route · 5.10 km"
    )
    await selectActiveRoute({ route_id: second.id })
    await act(async () => router.invalidate({ sync: true }))
    expect(
      document.querySelector('[aria-label="Active route"]')?.textContent
    ).toContain("4.75 km walking loop")
    expect(document.querySelector("figure")?.textContent).toContain(
      "Showing active route · 4.75 km"
    )
    expect(document.querySelector("figure")?.textContent).not.toContain(
      "Showing active route · 5.10 km"
    )
  } finally {
    await act(async () => root.unmount())

    if (originalLoader !== undefined)
      TravelRoute.options.loader = originalLoader

    await api.stop()
    vi.unstubAllGlobals()
    vi.unstubAllEnvs()
  }
})
