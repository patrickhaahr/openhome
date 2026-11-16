import { createServer } from "node:http"
import { expect, test } from "vitest"
import {
  apiKey,
  axumUrl,
  dashboard,
  listen,
  page,
} from "./support/connection-server"

test("guides the installer when private configuration is missing", async () => {
  const html = await page(await dashboard())
  expect(html).toContain("Connect your home.")
  expect(html).toContain("Setup needed")
  expect(html).toContain("OPENHOME_API_URL")
  expect(html).toContain("API_KEY")
  const home = await page(await dashboard(), "/")
  expect(home).toContain("Connect OpenHome before sending a light command.")
  expect(home).toMatch(
    /<button[^>]*\sdisabled[^>]*>(?:(?!<\/button>).)*<span class="sr-only">Turn on<\/span><\/button>/
  )
})

test("renders authenticated health from the real Axum route without exposing configuration", async () => {
  const html = await page(await dashboard({ url: axumUrl, key: apiKey }))
  expect(html).toContain("Your home is connected.")
  expect(html).toContain("Connected")
  expect(html).toContain("Check again")
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
