// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// This harness also executes the private server transport; CORS applies to browser RPCs, not Axum calls.

import { spawn, type ChildProcess } from "node:child_process"
import { once } from "node:events"
import { createServer, type Server } from "node:http"
import { resolve } from "node:path"
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  test,
  vi,
} from "vitest"
import { z } from "zod"
import { WalkingLoopPlanner } from "@/components/walking-loop"
import type { MapLine } from "@/lib/map"
import { generateLoop } from "@/lib/route-loop.server"

const apiKey = "synthetic-dashboard-api-key"

let api: ChildProcess | undefined

let apiUrl: string

let root: Root

let container: HTMLDivElement

let routes: Array<MapLine | undefined>

let planning: Promise<unknown>

const servers: Server[] = []

beforeAll(async () => {
  // The fixture's loopback provider serves a synthetic loop from the demo origin.
  const child = spawn(
    resolve("../api/target/debug/examples/dashboard_health"),
    [],
    {
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, OPENROUTESERVICE_URL: "" },
    }
  )

  api = child
  apiUrl = await new Promise<string>((resolve, reject) => {
    child.stdout?.on("data", (chunk: Buffer) => {
      const url = chunk.toString().match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]

      if (url !== undefined) resolve(url)
    })
    child.once("error", reject)
    child.once("exit", (code) => reject(new Error(`Axum exited: ${code}`)))
  })
})

afterAll(async () => {
  if (api !== undefined && api.exitCode === null && api.signalCode === null) {
    const exited = once(api, "exit")

    api.kill("SIGTERM")
    await exited
  }
})

beforeEach(() => {
  vi.stubEnv("OPENHOME_API_URL", apiUrl)
  vi.stubEnv("API_KEY", apiKey)
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
  routes = []
  planning = Promise.resolve()
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.unstubAllGlobals()
  vi.unstubAllEnvs()

  for (const server of servers.splice(0)) {
    server.closeAllConnections()
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve()))
    )
  }
})

async function render(): Promise<void> {
  await act(async () => {
    root.render(
      <WalkingLoopPlanner
        plan={(request) => {
          const work = generateLoop(request)
          planning = work

          return work
        }}
        onRouteChange={(route) => routes.push(route)}
      />
    )
  })
}

async function plan(label: string): Promise<void> {
  const button = Array.from(container.querySelectorAll("button")).find(
    (element) => element.textContent === label
  )

  if (button === undefined) throw new Error(`Missing button: ${label}`)

  await act(async () => {
    button.click()
    await planning
  })
}

/** An upstream standing in for Axum, answering successive loop requests in order. */
async function upstream(responses: ReadonlyArray<object>): Promise<string> {
  let next = 0

  const server = createServer((request, response) => {
    expect(request.headers.authorization).toBe(`Bearer ${apiKey}`)
    expect(request.url).toBe("/api/routes/loops")
    response.writeHead(200, { "Content-Type": "application/json" })
    response.end(JSON.stringify(responses[next]))
    next += 1
  })

  servers.push(server)
  server.listen(0, "127.0.0.1")
  await once(server, "listening")
  const address = z.object({ port: z.number() }).parse(server.address())

  return `http://127.0.0.1:${address.port}`
}

test("plans a measured 5 km loop through authenticated Axum and hands its geometry to the map", async () => {
  await render()
  expect(container.textContent).toContain("returns to the origin")

  await plan("Plan a 5 km walk")

  expect(container.textContent).toContain("5.10 km")
  expect(container.textContent).toContain(
    "100 m (2.0%) longer than the 5.00 km target, within ±10%"
  )
  expect(container.querySelector("button")).toBeNull()
  expect(routes).toHaveLength(1)
  const [route] = routes
  expect(route?.coordinates[0]).toEqual([12.5692, 55.6869])
  expect(route?.coordinates.at(-1)).toEqual([12.5692, 55.6869])
  expect(container.innerHTML).not.toContain(apiKey)
})

test("explains provider outcomes, never draws an invalid route and stops re-planning a shown loop", async () => {
  const generated = await generateLoop({
    activity: "walking",
    target_meters: 5000,
  })

  vi.stubEnv(
    "OPENHOME_API_URL",
    await upstream([
      { state: "unavailable", reason: "provider-unavailable" },
      { state: "unavailable", reason: "no-route" },
      // A route outside the contract is rejected, never drawn.
      {
        state: "generated",
        route: { geometry: { type: "LineString", coordinates: [[200, 95]] } },
      },
      generated,
    ])
  )
  await render()
  await plan("Plan a 5 km walk")
  expect(container.textContent).toContain("Route planner unavailable")
  expect(routes).toEqual([undefined])

  await plan("Try again")
  expect(container.textContent).toContain("No walkable loop found")
  expect(container.textContent).toContain("No route was invented.")

  await plan("Try again")
  expect(container.textContent).toContain("Route planning unavailable")
  expect(routes).toEqual([undefined, undefined, undefined])

  await plan("Try again")
  expect(container.textContent).toContain("measured loop")
  expect(routes.at(-1)?.type).toBe("LineString")
  // The same request plans the same loop, so no button spends another request.
  expect(container.querySelector("button")).toBeNull()
})

test("labels a loop outside the tolerance as a deviation, never a match", async () => {
  const generated = await generateLoop({
    activity: "walking",
    target_meters: 5000,
  })

  if (generated.state !== "generated") throw new Error("Expected a loop")

  vi.stubEnv(
    "OPENHOME_API_URL",
    await upstream([
      {
        state: "generated",
        route: {
          ...generated.route,
          distance_meters: 4300,
          deviation_meters: -700,
          deviation_percent: -14,
          within_tolerance: false,
        },
      },
    ])
  )
  await render()
  await plan("Plan a 5 km walk")
  expect(container.textContent).toContain("4.30 km")
  expect(container.textContent).toContain(
    "700 m (14.0%) shorter than the 5.00 km target, outside ±10%"
  )
  expect(container.textContent).not.toContain("within")
})
