import { spawn, type ChildProcess } from "node:child_process"
import { createServer, type Server } from "node:http"
import { once } from "node:events"
import { fileURLToPath } from "node:url"
import { afterAll, afterEach, beforeAll, expect, test } from "vitest"
import { z } from "zod"
import demoWeather from "./fixtures/weather.json"

const root = fileURLToPath(new URL("../", import.meta.url))

const apiKey = "synthetic-dashboard-api-key"

const processes: ChildProcess[] = []

const servers: Server[] = []

let axumUrl: string

async function startProcess({
  command,
  args,
  env,
}: {
  command: string
  args: string[]
  env: NodeJS.ProcessEnv
}): Promise<string> {
  const child = spawn(command, args, {
    cwd: root,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  })

  processes.push(child)

  return new Promise((resolve, reject) => {
    let output = ""

    const timeout = setTimeout(
      () => reject(new Error(`Server startup timed out: ${output}`)),
      20000
    )

    child.stdout.on("data", (chunk: Buffer) => {
      output += chunk.toString()
      const url = output.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]

      if (url !== undefined) {
        clearTimeout(timeout)
        resolve(url)
      }
    })
    child.stderr.on("data", (chunk: Buffer) => {
      output += chunk.toString()
    })
    child.once("error", (error) => {
      clearTimeout(timeout)
      reject(error)
    })
    child.once("exit", (code) => {
      clearTimeout(timeout)
      reject(new Error(`Server exited (${code}): ${output}`))
    })
  })
}

async function dashboard({
  url = "",
  key = "",
}: { url?: string; key?: string } = {}): Promise<string> {
  return startProcess({
    command: "bun",
    args: [
      "node_modules/vite/bin/vite.js",
      "dev",
      "--host",
      "127.0.0.1",
      "--port",
      "0",
      "--clearScreen",
      "false",
    ],
    env: { ...process.env, OPENHOME_API_URL: url, API_KEY: key },
  })
}

async function page(url: string, path = "/connection"): Promise<string> {
  const response = await fetch(`${url}${path}`)
  expect(response.status).toBe(200)
  expect(response.headers.get("cache-control")).toBe("no-store")
  const html = await response.text()
  expect(html).not.toContain(apiKey)
  expect(html).not.toContain(axumUrl)

  return html
}

beforeAll(async () => {
  axumUrl = await startProcess({
    command: fileURLToPath(
      new URL(
        "../../api/target/debug/examples/dashboard_health",
        import.meta.url
      )
    ),
    args: [],
    env: process.env,
  })
})

async function cleanup(children: ChildProcess[]): Promise<void> {
  for (const child of children) {
    if (child.exitCode === null && child.signalCode === null) {
      const exited = once(child, "exit")
      child.kill("SIGTERM")
      await exited
    }
  }

  for (const server of servers.splice(0)) {
    server.closeAllConnections()
    await new Promise<void>((resolve, reject) => {
      server.close((error) => {
        if (error !== undefined) reject(error)
        else resolve()
      })
    })
  }
}

afterEach(() => cleanup(processes.splice(1)))

afterAll(() => cleanup(processes.splice(0)))

async function listen(server: Server): Promise<string> {
  servers.push(server)
  server.listen(0, "127.0.0.1")
  await once(server, "listening")
  const address = z.object({ port: z.number() }).parse(server.address())

  return `http://127.0.0.1:${address.port}`
}

test("guides the installer when private configuration is missing", async () => {
  const html = await page(await dashboard())
  expect(html).toContain("Connect your home.")
  expect(html).toContain("Setup needed")
  expect(html).toContain("OPENHOME_API_URL")
  expect(html).toContain("API_KEY")
  const home = await page(await dashboard(), "/")
  expect(home).toContain("Connect OpenHome before sending a light command.")
  expect(home).toMatch(/<button[^>]*\sdisabled[^>]*>Turn on<\/button>/)
})

test("renders authenticated health from the real Axum route without exposing configuration", async () => {
  const html = await page(await dashboard({ url: axumUrl, key: apiKey }))
  expect(html).toContain("Your home is connected.")
  expect(html).toContain("Connected")
  expect(html).toContain("Check again")
})

test("renders Home and a focused deep link without treating healthy Axum as connected integrations", async () => {
  const url = await dashboard({ url: axumUrl, key: apiKey })
  const home = await page(url, "/")
  expect(home).toContain("Home connected")
  expect(home).toContain("Current time")
  expect(home).toContain("Copenhagen time")
  expect(home).toContain("Weather unavailable")
  expect(home).toContain("Turn on")
  expect(home).toContain("Turn off")
  expect(home).toContain("Observed state")
  expect(home).toContain("Unknown")

  const photos = await page(url, "/photos")
  expect(photos).toContain("No photo library is connected here")
  expect(photos).toContain("Back to Home")
})

test("renders a rejected credential from Axum authentication", async () => {
  const rejectedKey = "synthetic-rejected-key"
  const html = await page(await dashboard({ url: axumUrl, key: rejectedKey }))
  expect(html).toContain("API key rejected")
  expect(html).not.toContain(rejectedKey)
})

test.each([
  "http://synthetic-private-host.invalid/?token=synthetic-secret",
  "http://synthetic-user:synthetic-password@synthetic-private-host.invalid",
  "file:///synthetic-private-path",
])("reports invalid configuration without reflecting it: %s", async (url) => {
  const html = await page(await dashboard({ url, key: apiKey }))
  expect(html).toContain("Check setup")
  expect(html).not.toContain("synthetic-private")
  expect(html).not.toContain("synthetic-secret")
  expect(html).not.toContain("synthetic-password")
})

test("reports network failure distinctly and recovers when Axum returns", async () => {
  let online = false

  const proxy = createServer((request, response) => {
    if (!online) {
      request.socket.destroy()

      return
    }

    void fetch(`${axumUrl}/api/health`, {
      headers: { Authorization: request.headers.authorization ?? "" },
    }).then(async (upstream) => {
      response.writeHead(upstream.status, {
        "Content-Type": "application/json",
      })
      response.end(await upstream.text())
    })
  })

  const url = await dashboard({
    url: await listen(proxy),
    key: apiKey,
  })

  expect(await page(url)).toContain("Unreachable")
  online = true
  expect(await page(url)).toContain("Your home is connected.")
})

test.each([
  {
    status: 503,
    body: "synthetic-private-error-with-key",
    expected: "Unavailable",
  },
  {
    status: 200,
    body: "not JSON: synthetic-private-error-with-key",
    expected: "Unavailable",
  },
  {
    status: 200,
    body: '{"status":"down","detail":"synthetic-private-error-with-key"}',
    expected: "Unavailable",
  },
  {
    status: 200,
    body: '{"status":"ok","detail":"synthetic-private-error-with-key"}',
    expected: "Your home is connected.",
  },
])(
  "classifies backend responses without reflecting private fields ($status, $body)",
  async ({ status, body, expected }) => {
    const upstream = createServer((_request, response) => {
      response.writeHead(status, { "Content-Type": "application/json" })
      response.end(body)
    })

    const privateUrl = await listen(upstream)
    const html = await page(await dashboard({ url: privateUrl, key: apiKey }))
    expect(html).toContain(expected)
    expect(html).not.toContain("synthetic-private-error-with-key")
    expect(html).not.toContain(privateUrl)
  }
)

test("bounds a stalled health check so the connection screen remains usable", async () => {
  const upstream = createServer(() => {
    /* Deliberately leave the response pending. */
  })

  const privateUrl = await listen(upstream)
  const url = await dashboard({ url: privateUrl, key: apiKey })
  const started = Date.now()
  const html = await page(url)

  expect(html).toContain("Unreachable")
  expect(html).toContain("Check again")
  expect(Date.now() - started).toBeLessThan(10000)
  expect(html).not.toContain(privateUrl)
})

test("renders current and seven-day weather through the private server boundary without exposing upstream fields", async () => {
  const forecast = structuredClone(demoWeather)
  forecast.forecast.days[2] = {
    date: "2026-10-13",
    weather_code: 255,
    high_c: 13,
    low_c: 6.8,
    precipitation_mm: 4.9,
    wind_ms: 9.3,
  }
  const requests: string[] = []

  const upstream = createServer((request, response) => {
    expect(request.headers.authorization).toBe(`Bearer ${apiKey}`)
    requests.push(request.url ?? "")
    response.writeHead(200, { "Content-Type": "application/json" })
    response.end(
      JSON.stringify(
        request.url === "/api/weather"
          ? {
              ...forecast,
              latitude: "synthetic-private-location",
              forecast: {
                ...forecast.forecast,
                location: "synthetic-private-location",
              },
            }
          : request.url === "/api/health"
            ? { status: "ok" }
            : { remotes: { lgtv: ["power"], edifier: ["bluetooth"] } }
      )
    )
  })

  const privateUrl = await listen(upstream)
  const url = await dashboard({ url: privateUrl, key: apiKey })
  const home = await page(url, "/")
  expect(home).toContain("9,3 °C")
  expect(home).toContain("Overcast")
  expect(home).toContain("Seven-day forecast")
  expect(home).toContain("Turn on")
  const weather = await page(url, "/weather")
  expect(weather).toContain("Fresh forecast")
  expect(weather).toMatch(/11\. okt\.[, ]+12[.:]00/)
  expect(weather).toContain("6,6 °C")
  expect(weather).toContain("3,3 m/s")
  expect(weather).toContain("0,9 mm")
  expect(weather).toContain("Conditions unavailable")
  expect(weather.match(/<time dateTime=/g)).toHaveLength(7)
  expect(weather).toContain("2026-10-17")
  expect(weather).toContain("Open-Meteo")
  expect(weather).toContain("CC BY 4.0")
  expect(requests.filter((path) => path === "/api/weather")).toHaveLength(2)

  for (const html of [home, weather]) {
    expect(html).not.toContain("synthetic-private-location")
    expect(html).not.toContain(privateUrl)
  }
})

test.each([
  { state: "stale", expected: "Stale forecast" },
  { state: "unavailable", expected: "Weather setup needed" },
  { state: "invalid", expected: "Weather unavailable" },
])(
  "renders honest weather $state states through the application boundary",
  async ({ state, expected }) => {
    const upstream = createServer((_request, response) => {
      response.writeHead(200, { "Content-Type": "application/json" })
      response.end(
        JSON.stringify(
          state === "unavailable"
            ? { state, reason: "missing-configuration" }
            : state === "invalid"
              ? { state: "fresh", forecast: "synthetic-private-error" }
              : {
                  ...demoWeather,
                  state,
                  forecast: {
                    ...demoWeather.forecast,
                    current: null,
                    days: demoWeather.forecast.days.map((day) => ({
                      ...day,
                      precipitation_mm: null,
                    })),
                  },
                }
        )
      )
    })

    const weather = await page(
      await dashboard({ url: await listen(upstream), key: apiKey }),
      "/weather"
    )

    expect(weather).toContain(expected)
    expect(weather).not.toContain("synthetic-private-error")

    if (state === "stale") {
      expect(weather).toContain("Current conditions unavailable")
      expect(weather).toContain("conditions may have changed")
      expect(weather).toContain("Fetched")
      expect(weather).toMatch(/11\. okt\.[, ]+12[.:]00/)
      expect(weather).toContain("Precipitation</dt><dd>Unavailable")
      expect(weather).not.toContain("0 mm")
    }
  }
)

test("streams usable Home controls before a stalled weather request completes", async () => {
  const upstream = createServer((request, response) => {
    if (request.url === "/api/weather") return
    response.writeHead(200, { "Content-Type": "application/json" })
    response.end(
      JSON.stringify(
        request.url === "/api/health"
          ? { status: "ok" }
          : { remotes: { lgtv: [], edifier: [] } }
      )
    )
  })

  const url = await dashboard({ url: await listen(upstream), key: apiKey })
  const started = Date.now()

  const response = await fetch(`${url}/`, {
    headers: {
      "User-Agent":
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/17.0 Safari/605.1.15",
    },
  })

  const reader = response.body?.getReader()

  if (reader === undefined) throw new Error("Expected streamed Home")

  const decoder = new TextDecoder()
  let html = ""

  try {
    while (!html.includes("Turn on")) {
      const chunk = await reader.read()

      if (chunk.done) throw new Error("Home ended before rendering controls")

      html += decoder.decode(chunk.value, { stream: true })
    }

    expect(html).toContain("Checking weather…")
    expect(Date.now() - started).toBeLessThan(5000)
  } finally {
    await reader.cancel()
  }
})
