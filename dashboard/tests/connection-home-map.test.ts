import { createServer } from "node:http"
import { expect, test } from "vitest"
import {
  apiKey,
  axumUrl,
  dashboard,
  listen,
  page,
} from "./support/connection-server"

test("renders Home and a focused deep link without treating healthy Axum as connected integrations", async () => {
  const url = await dashboard({ url: axumUrl, key: apiKey })
  const home = await page(url, "/")
  expect(home).toContain("Current time")
  expect(home).toContain("Copenhagen time")
  expect(home).toContain("Weather unavailable")
  expect(home).toContain("Turn on")
  expect(home).toContain("Turn off")
  expect(home).toContain("Observed state")
  expect(home).toContain("Unknown")

  const photos = await page(url, "/photos")
  expect(photos).toContain("Photo setup needed")
  expect(photos).toContain("No photo library is connected here")
})

test("opens the map at the explicit demo origin served by the real Axum route", async () => {
  const travel = await page(
    await dashboard({ url: axumUrl, key: apiKey }),
    "/travel"
  )

  expect(travel).toContain("Routes &amp; transit")
  expect(travel).toContain("Demo origin · central Copenhagen")
  expect(travel).toContain("Map of the Copenhagen demo area")
  expect(travel).toContain("Loading map…")
  expect(travel).toContain("Stops and live vehicles are not connected yet.")
  expect(travel).toContain("Plan loops")
  expect(travel).toMatch(
    /<link rel="stylesheet" href="[^"]*maplibre-gl[^"]*\.css/
  )
  expect(travel).not.toContain("Private origin")
})

test("labels a private map origin and keeps setup and connection failures explicit", async () => {
  let body = JSON.stringify({
    state: "configured",
    origin: { latitude: 55.5, longitude: 12.25, source: "private" },
    label: "synthetic-private-label",
  })

  const upstream = createServer((request, response) => {
    expect(request.headers.authorization).toBe(`Bearer ${apiKey}`)

    if (request.url !== "/api/map") {
      response.writeHead(404).end()

      return
    }

    response.writeHead(200, { "Content-Type": "application/json" })
    response.end(body)
  })

  const url = await dashboard({ url: await listen(upstream), key: apiKey })
  const configured = await page(url, "/travel")
  expect(configured).toContain("Private origin")
  expect(configured).toContain("Map of the local area")
  expect(configured).not.toContain("Demo origin")
  expect(configured).not.toContain("synthetic-private-label")

  body = JSON.stringify({
    state: "unavailable",
    reason: "missing-configuration",
  })
  const missing = await page(url, "/travel")
  expect(missing).toContain("Map setup needed")
  expect(missing).toContain("There is no default location.")
  expect(missing).not.toContain("Loading map…")

  body = JSON.stringify({ state: "configured", origin: { latitude: "north" } })
  const malformed = await page(url, "/travel")
  expect(malformed).toContain("Map unavailable")
  expect(malformed).toContain("Check Connection or try again.")

  const unconfigured = await page(await dashboard(), "/travel")
  expect(unconfigured).toContain("Map unavailable")
  expect(unconfigured).not.toContain("Loading map…")
})
