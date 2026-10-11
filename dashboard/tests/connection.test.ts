import { spawn, type ChildProcess } from "node:child_process"
import { createServer, type Server } from "node:http"
import { once } from "node:events"
import { fileURLToPath } from "node:url"
import { afterAll, afterEach, beforeAll, expect, test } from "vitest"
import { z } from "zod"

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
  expect(home).toContain("Turn on")
  expect(home).toContain("disabled")
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
  expect(home).toContain("Not available yet")
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
