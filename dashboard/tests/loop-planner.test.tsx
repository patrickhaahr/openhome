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
import { LoopPlanner } from "@/components/loop-planner"
import type { MapRoutes } from "@/lib/map"
import type { LoopRequest } from "@/lib/route-loop"
import { generateLoops } from "@/lib/route-loop.server"

const apiKey = "synthetic-dashboard-api-key"

const FIVE_KM: LoopRequest = {
  activity: "walking",
  target: { kind: "distance", meters: 5000 },
  step_length_cm: 75,
}

let api: ChildProcess | undefined

let apiUrl: string

let root: Root

let container: HTMLDivElement

let routes: MapRoutes[]

let requests: LoopRequest[]

let planning: Promise<unknown>

const servers: Server[] = []

beforeAll(async () => {
  // The fixture's loopback provider answers each seed with a synthetic square from the demo origin.
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
  requests = []
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
      <LoopPlanner
        plan={(request) => {
          requests.push(request)
          const work = generateLoops(request)
          planning = work

          return work
        }}
        onRoutesChange={(next) => routes.push(next)}
      />
    )
  })
}

async function click(label: string): Promise<void> {
  const button = Array.from(container.querySelectorAll("button")).find(
    (element) => element.textContent?.startsWith(label) === true
  )

  if (button === undefined) throw new Error(`Missing button: ${label}`)

  await act(async () => {
    button.click()
    await planning
  })
}

async function type(label: string, value: string): Promise<void> {
  const fieldLabel = Array.from(container.querySelectorAll("label")).find(
    (element) => element.textContent === label
  )

  const input = document.getElementById(fieldLabel?.htmlFor ?? "")

  if (!(input instanceof HTMLInputElement))
    throw new Error(`Missing field: ${label}`)

  await act(async () => {
    // Native setter represents typing rather than replacing React's tracked value.
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value"
    )?.set?.call(input, value)
    input.dispatchEvent(new Event("input", { bubbles: true }))
  })
}

function alternatives(): HTMLButtonElement[] {
  return Array.from(
    container.querySelectorAll<HTMLButtonElement>(".loop-alternative")
  )
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

async function generatedPlan() {
  const read = await generateLoops(FIVE_KM)

  if (read.state !== "generated") throw new Error("Expected loops")

  return read.plan
}

test("compares three measured 5 km walking loops from authenticated Axum on the map", async () => {
  await render()
  expect(container.textContent).toContain("Up to three loops")

  await click("Plan loops")

  expect(requests).toEqual([FIVE_KM])
  expect(container.textContent).toContain("Found 3 loops.")
  expect(container.textContent).toContain("5.00 km walking loops")
  const [first, second, third] = alternatives()
  expect(first?.textContent).toContain("Loop A5.10 km")
  expect(first?.textContent).toContain(
    "100 m (2.0%) longer than 5.00 km, within ±10%"
  )
  expect(first?.textContent).toContain(
    "About 6,800 steps, estimated from a 75 cm step"
  )
  expect(second?.textContent).toContain("shorter than 5.00 km, within ±10%")
  expect(third?.textContent).toContain("longer than 5.00 km, within ±10%")
  expect(first?.getAttribute("aria-pressed")).toBe("true")

  // Every alternative is drawn from the origin and back; the first is highlighted.
  expect(routes).toHaveLength(1)
  const [drawn] = routes
  expect(drawn?.focused).toBe(0)
  expect(drawn?.lines).toHaveLength(3)

  for (const line of drawn?.lines ?? []) {
    expect(line.coordinates[0]).toEqual([12.5692, 55.6869])
    expect(line.coordinates.at(-1)).toEqual([12.5692, 55.6869])
  }

  await click("Loop B")
  expect(second?.getAttribute("aria-pressed")).toBe("true")
  expect(first?.getAttribute("aria-pressed")).toBe("false")
  expect(routes.at(-1)).toEqual({ lines: drawn?.lines, focused: 1 })
  expect(container.innerHTML).not.toContain(apiKey)
})

test("plans running loops from a step target and a running step length", async () => {
  await render()
  await click("Run")
  await click("Steps")
  await type("Steps", "10000")
  await type("Running step length in cm", "100")
  await click("Plan loops")

  expect(requests).toEqual([
    {
      activity: "running",
      target: { kind: "steps", steps: 10_000 },
      step_length_cm: 100,
    },
  ])
  expect(container.textContent).toContain(
    "10,000 steps (10.00 km) running loops"
  )
  expect(container.textContent).toContain("estimated from a 100 cm step")
  expect(alternatives()).toHaveLength(3)
})

test("shows Axum's reason for an unsupported target and asks for missing input", async () => {
  await render()
  await click("Custom")
  await type("Distance in km", "30")
  await click("Plan loops")
  expect(container.textContent).toContain("Target not accepted")
  expect(container.textContent).toContain(
    "Target distance must be between 1 km and 25 km, in whole metres"
  )

  await click("Steps")
  await type("Steps", "")
  await click("Plan loops")
  expect(container.textContent).toContain("Enter a whole number of steps.")
  expect(requests).toHaveLength(1)
  expect(routes).toEqual([])
})

test("keeps earlier loops usable when a later request fails, and never draws an invalid route", async () => {
  const plan = await generatedPlan()

  vi.stubEnv(
    "OPENHOME_API_URL",
    await upstream([
      { state: "generated", plan },
      { state: "unavailable", reason: "provider-unavailable" },
      // A route outside the contract is rejected, never drawn.
      {
        state: "generated",
        plan: {
          ...plan,
          loops: [
            { geometry: { type: "LineString", coordinates: [[200, 95]] } },
          ],
        },
      },
    ])
  )
  await render()
  await click("Plan loops")
  expect(alternatives()).toHaveLength(3)

  await click("10 km")
  await click("Plan loops")
  expect(container.textContent).toContain("Route planner unavailable")
  expect(container.textContent).toContain(
    "Still showing the earlier 5.00 km walking loops."
  )
  expect(alternatives()).toHaveLength(3)

  await click("Plan loops")
  expect(container.textContent).toContain("Route planning unavailable")
  expect(container.textContent).toContain("Still showing the earlier")
  // Only the first, valid answer reached the map.
  expect(routes).toHaveLength(1)
})

test("explains too few matching loops and labels deviations without calling them matches", async () => {
  const plan = await generatedPlan()
  const [match] = plan.loops

  if (match === undefined) throw new Error("Expected a loop")

  vi.stubEnv(
    "OPENHOME_API_URL",
    await upstream([
      {
        state: "generated",
        plan: {
          ...plan,
          search: "exhausted",
          loops: [
            match,
            {
              ...match,
              distance_meters: 4300,
              deviation_meters: -700,
              deviation_percent: -14,
              within_tolerance: false,
            },
          ],
        },
      },
    ])
  )
  await render()
  await click("Plan loops")
  expect(container.textContent).toContain(
    "The search used all its attempts. Only 1 distinct loop came within ±10% of the target. The closest others are shown with their deviation."
  )
  const [, deviation] = alternatives()
  expect(deviation?.textContent).toContain("4.30 km")
  expect(deviation?.textContent).toContain(
    "700 m (14.0%) shorter than 5.00 km, outside ±10%"
  )
  expect(deviation?.textContent).not.toContain("within")
})
