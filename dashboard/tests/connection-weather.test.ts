import { createServer } from "node:http"
import { expect, test } from "vitest"
import demoWeather from "./fixtures/weather.json"
import { apiKey, dashboard, listen, page } from "./support/connection-server"

test("renders current, hourly and seven-day weather through the private server boundary without exposing upstream fields", async () => {
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
  expect(home).toContain("9°")
  expect(home).toContain("Overcast")
  expect(home).toContain('aria-label="Open Weather"')
  expect(home).toContain("Turn on")
  const weather = await page(url, "/weather")
  expect(weather).toContain("Fresh forecast")
  expect(weather).toMatch(/11 Oct[, ]+12:00/)
  expect(weather).toContain("Conditions unavailable")
  expect(weather.match(/<time dateTime=/g)).toHaveLength(31)
  expect(weather).toContain("Walking or running?")
  expect(weather).toContain("Next 24 hours")
  expect(weather).toContain("60 %")
  expect(weather).toContain("Hourly conditions unavailable")
  expect(weather).toContain("2026-10-12T09:00:00Z")
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
  { start: "2026-03-29T00:00:00Z", labels: ["01", "03"] },
  { start: "2026-10-25T00:00:00Z", labels: ["02", "02"] },
])(
  "renders distinct Copenhagen hours across DST at $start",
  async ({ start, labels }) => {
    const upstream = createServer((_request, response) => {
      response.writeHead(200, { "Content-Type": "application/json" })
      response.end(
        JSON.stringify({
          ...demoWeather,
          forecast: {
            ...demoWeather.forecast,
            hourly: demoWeather.forecast.hourly.map((hour, index) => ({
              ...hour,
              starts_at: new Date(
                Date.parse(start) + index * 3_600_000
              ).toISOString(),
            })),
          },
        })
      )
    })

    const html = await page(
      await dashboard({ url: await listen(upstream), key: apiKey }),
      "/weather"
    )

    for (const [index, label] of labels.entries()) {
      const instant = new Date(
        Date.parse(start) + index * 3_600_000
      ).toISOString()

      expect(html).toContain(`<time dateTime="${instant}">${label}:00</time>`)
    }

    expect(html).toContain("GMT+1")
    expect(html).toContain("GMT+2")
  }
)

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
                    hourly: [],
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
      expect(weather).toContain("Hourly forecast unavailable")
      expect(weather).toContain("conditions may have changed")
      expect(weather).toContain("Fetched")
      expect(weather).toMatch(/11 Oct[, ]+12:00/)
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
